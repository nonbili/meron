//! Synchronous facade over oo7's async, portal-backed keyring.
//!
//! The sidecar's storage hooks are synchronous and can run from inside its
//! Tokio runtime, where calling `block_on` directly would panic. A dedicated
//! worker owns one oo7 keyring and serializes all file-backed operations.

use std::collections::HashMap;
use std::sync::{OnceLock, mpsc};

use anyhow::{Context, Result, anyhow};

const SERVICE: &str = "meron";

enum Request {
    Store {
        account: String,
        value: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
    Load {
        account: String,
        reply: mpsc::Sender<Result<Option<String>, String>>,
    },
    Delete {
        account: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
}

impl Request {
    fn fail(self, error: String) {
        match self {
            Self::Store { reply, .. } | Self::Delete { reply, .. } => {
                let _ = reply.send(Err(error));
            }
            Self::Load { reply, .. } => {
                let _ = reply.send(Err(error));
            }
        }
    }
}

struct PortalKeyring {
    requests: mpsc::Sender<Request>,
}

static PORTAL_KEYRING: OnceLock<PortalKeyring> = OnceLock::new();

fn portal_keyring() -> &'static PortalKeyring {
    PORTAL_KEYRING.get_or_init(|| {
        let (requests, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("meron-portal-keyring".into())
            .spawn(move || worker(receiver))
            .expect("failed to start portal keyring worker");
        PortalKeyring { requests }
    })
}

fn worker(receiver: mpsc::Receiver<Request>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            for request in receiver {
                request.fail(format!("create portal keyring runtime: {error}"));
            }
            return;
        }
    };
    let mut keyring = None;

    for request in receiver {
        if keyring.is_none() {
            match runtime.block_on(oo7::Keyring::new()) {
                Ok(created) => keyring = Some(created),
                Err(error) => {
                    request.fail(format!("open portal keyring: {error}"));
                    continue;
                }
            }
        }
        let keyring = keyring.as_ref().expect("portal keyring initialized");

        match request {
            Request::Store {
                account,
                value,
                reply,
            } => {
                let result = runtime
                    .block_on(store_async(keyring, &account, &value))
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
            Request::Load { account, reply } => {
                let result = runtime
                    .block_on(load_async(keyring, &account))
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
            Request::Delete { account, reply } => {
                let result = runtime
                    .block_on(delete_async(keyring, &account))
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
        }
    }
}

fn attributes(account: &str) -> HashMap<&str, &str> {
    HashMap::from([("service", SERVICE), ("username", account)])
}

async fn store_async(keyring: &oo7::Keyring, account: &str, value: &str) -> oo7::Result<()> {
    keyring
        .create_item(
            &format!("{account}@{SERVICE}"),
            &attributes(account),
            oo7::Secret::text(value),
            true,
        )
        .await
}

async fn load_async(keyring: &oo7::Keyring, account: &str) -> Result<Option<String>> {
    let Some(item) = keyring.search_items(&attributes(account)).await?.pop() else {
        return Ok(None);
    };
    let secret = item.secret().await?;
    String::from_utf8(secret.as_bytes().to_vec())
        .map(Some)
        .context("portal keyring entry is not UTF-8")
}

async fn delete_async(keyring: &oo7::Keyring, account: &str) -> oo7::Result<()> {
    keyring.delete(&attributes(account)).await
}

fn response<T>(receiver: mpsc::Receiver<Result<T, String>>) -> Result<T> {
    receiver
        .recv()
        .map_err(|_| anyhow!("portal keyring worker stopped"))?
        .map_err(anyhow::Error::msg)
}

pub fn store(account: &str, value: &str) -> Result<()> {
    let (reply, receiver) = mpsc::channel();
    portal_keyring()
        .requests
        .send(Request::Store {
            account: account.to_owned(),
            value: value.to_owned(),
            reply,
        })
        .map_err(|_| anyhow!("portal keyring worker stopped"))?;
    response(receiver)
}

pub fn load(account: &str) -> Result<Option<String>> {
    let (reply, receiver) = mpsc::channel();
    portal_keyring()
        .requests
        .send(Request::Load {
            account: account.to_owned(),
            reply,
        })
        .map_err(|_| anyhow!("portal keyring worker stopped"))?;
    response(receiver)
}

pub fn delete(account: &str) -> Result<()> {
    let (reply, receiver) = mpsc::channel();
    portal_keyring()
        .requests
        .send(Request::Delete {
            account: account.to_owned(),
            reply,
        })
        .map_err(|_| anyhow!("portal keyring worker stopped"))?;
    response(receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_attributes_are_app_scoped() {
        let attributes = attributes("account-1");
        assert_eq!(attributes.get("service"), Some(&"meron"));
        assert_eq!(attributes.get("username"), Some(&"account-1"));
    }
}
