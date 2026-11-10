package jp.nonbili.meron;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import androidx.test.platform.app.InstrumentationRegistry;

import org.json.JSONObject;
import org.junit.Test;

import java.io.File;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

public final class MeronCoreNativeContractTest {
    @Test
    public void openPgpLabSendAndDraft() throws Exception {
        requireIsolatedRunner();
        android.os.Bundle args = InstrumentationRegistry.getArguments();
        String email = args.getString("pgpLabEmail", "");
        org.junit.Assume.assumeFalse("Opt-in local mail lab", email.isEmpty());
        File dir = new File(InstrumentationRegistry.getInstrumentation().getTargetContext().getCacheDir(),
                "openpgp-lab-" + System.nanoTime());
        assertTrue(dir.mkdirs());
        assertTrue(new JSONObject(MeronCoreNative.initJson(dir.getAbsolutePath())).getBoolean("ok"));
        JSONObject account = invoke("account.addPassword", new JSONObject()
                .put("email", email).put("username", email).put("password", args.getString("pgpLabPassword"))
                .put("imap_host", "127.0.0.1").put("imap_port", 3143)
                .put("smtp_host", "127.0.0.1").put("smtp_port", 3587)
                .put("tls", false).put("imap_tls", false).put("smtp_tls", false)
                .put("starttls", false).put("smtp_starttls", false)).getJSONObject("account");
        String id = account.getString("id");
        invoke("pgp.account.create", new JSONObject().put("account", id));
        JSONObject message = new JSONObject().put("account", id).put("to", email)
                .put("subject", "Meron Android OpenPGP lab " + System.nanoTime()).put("body", "android-openpgp-lab-secret")
                .put("encrypt", true).put("message_id", "<android-pgp-" + System.nanoTime() + "@mail.localhost>");
        assertTrue(invoke("mail.send", message).getBoolean("ok"));
        message.put("draft_id", "<android-pgp-draft-" + System.nanoTime() + "@mail.localhost>");
        assertTrue(invoke("mail.saveDraft", message).getBoolean("ok"));
        invoke("mail.sync", new JSONObject().put("account_id", id).put("folder_id", "INBOX"));
        org.json.JSONArray threads = invoke("mail.threadList", new JSONObject()
                .put("account_id", id).put("folder_id", "INBOX")).getJSONArray("threads");
        boolean decrypted = false;
        for (int index = 0; index < threads.length(); index++) {
            JSONObject thread = threads.getJSONObject(index);
            String threadId = thread.optString("thread_id", thread.optString("id"));
            org.json.JSONArray messages = invoke("mail.threadRead", new JSONObject()
                    .put("thread_id", threadId)).getJSONArray("messages");
            for (int n = 0; n < messages.length(); n++) {
                JSONObject read = messages.getJSONObject(n);
                if (read.optString("body").contains("android-openpgp-lab-secret")) {
                    assertTrue(read.getJSONObject("crypto").getBoolean("encrypted"));
                    assertTrue(read.getJSONObject("crypto").getBoolean("signed"));
                    decrypted = true;
                }
            }
        }
        assertTrue("Encrypted mail decrypted after SMTP/IMAP round trip", decrypted);
        org.json.JSONArray drafts = invoke("mail.threadList", new JSONObject()
                .put("account_id", id).put("folder_id", "Drafts")).getJSONArray("threads");
        boolean decryptedDraft = false;
        for (int index = 0; index < drafts.length(); index++) {
            JSONObject thread = drafts.getJSONObject(index);
            org.json.JSONArray messages = invoke("mail.threadRead", new JSONObject()
                    .put("thread_id", thread.optString("thread_id", thread.optString("id")))).getJSONArray("messages");
            for (int n = 0; n < messages.length(); n++) {
                JSONObject read = messages.getJSONObject(n);
                if (read.optString("body").contains("android-openpgp-lab-secret")) {
                    assertTrue(read.getJSONObject("crypto").getBoolean("encrypted"));
                    decryptedDraft = true;
                }
            }
        }
        assertTrue("Encrypted draft decrypted after IMAP round trip", decryptedDraft);
    }

    private JSONObject invoke(String method, JSONObject params) throws Exception {
        JSONObject request = new JSONObject().put("id", 71).put("method", method).put("params", params);
        JSONObject response = new JSONObject(MeronCoreNative.invokeJson(request.toString()));
        assertFalse("Core error for " + method + ": " + response.optJSONObject("error"), response.has("error"));
        return response.getJSONObject("result");
    }

    private void requireIsolatedRunner() {
        org.junit.Assume.assumeTrue("Requires IsolatedCoreTestRunner to protect installed profiles",
                InstrumentationRegistry.getInstrumentation() instanceof IsolatedCoreTestRunner);
    }

    @Test
    public void openPgpKeysRoundTripThroughPackagedMobileCore() throws Exception {
        requireIsolatedRunner();
        File dir = new File(InstrumentationRegistry.getInstrumentation().getTargetContext().getCacheDir(),
                "openpgp-contract-" + System.nanoTime());
        assertTrue(dir.mkdirs());
        assertTrue(new JSONObject(MeronCoreNative.initJson(dir.getAbsolutePath())).getBoolean("ok"));
        // Local account creation does not connect to a server.
        JSONObject account = invoke("account.addPassword", new JSONObject()
                .put("email", "mobile-pgp@example.org").put("username", "mobile-pgp@example.org")
                .put("password", "test-only").put("imap_host", "localhost").put("smtp_host", "localhost"))
                .getJSONObject("account");
        JSONObject params = new JSONObject().put("account", account.getString("id"));
        assertFalse(invoke("pgp.account.state", params).getBoolean("enabled"));
        String fingerprint = invoke("pgp.account.create", params).getJSONObject("key").getString("fingerprint");
        assertTrue(invoke("pgp.account.state", params).getBoolean("enabled"));
        assertTrue(invoke("pgp.account.exportPublic", params).getString("armored").contains("BEGIN PGP PUBLIC KEY"));
        JSONObject export = new JSONObject(params.toString()).put("passphrase", "mobile-test-passphrase");
        String secret = invoke("pgp.account.exportSecret", export).getString("armored");
        assertTrue(secret.contains("BEGIN PGP PRIVATE KEY"));
        invoke("pgp.account.disable", params);
        assertFalse(invoke("pgp.account.state", params).getBoolean("enabled"));
        JSONObject imported = invoke("pgp.account.importKey", new JSONObject(params.toString())
                .put("data", secret).put("passphrase", "mobile-test-passphrase")).getJSONObject("result");
        assertEquals("imported", imported.getString("status"));
        assertEquals(fingerprint, imported.getJSONObject("key").getString("fingerprint"));
        assertTrue(invoke("pgp.account.state", params).getBoolean("enabled"));
    }

    @Test
    public void packagedRustCoreMatchesProtocolAndHandlesPing() throws Exception {
        assertTrue("meron_core load error: " + MeronCoreNative.loadError(), MeronCoreNative.isLoaded());
        assertEquals(1, MeronCoreNative.protocolVersion());

        File dataDir = InstrumentationRegistry.getInstrumentation()
                .getTargetContext()
                .getFilesDir();
        JSONObject init = new JSONObject(MeronCoreNative.initJson(dataDir.getAbsolutePath()));
        assertTrue(init.getBoolean("ok"));
        assertEquals(1, init.getInt("protocol"));
        assertEquals(dataDir.getAbsolutePath(), init.getString("data_dir"));

        JSONObject ping = new JSONObject(MeronCoreNative.invokeJson("{\"id\":41,\"method\":\"ping\",\"params\":{}}"));
        assertEquals(41, ping.getInt("id"));
        assertTrue(ping.getJSONObject("result").getBoolean("pong"));
        assertEquals(1, ping.getJSONObject("result").getInt("protocol"));
    }

    @Test
    public void packagedRustCoreDeliversReadyEventThroughJni() throws Exception {
        assertTrue("meron_core load error: " + MeronCoreNative.loadError(), MeronCoreNative.isLoaded());

        CountDownLatch ready = new CountDownLatch(1);
        AtomicReference<String> eventJson = new AtomicReference<>("");
        MeronCoreNative.CoreEventListener listener = event -> {
            eventJson.set(event);
            ready.countDown();
        };

        MeronCoreNative.addCoreEventListener(listener);
        try {
            assertTrue(MeronCoreNative.emitReadyEvent());
            assertTrue("ready event was not delivered", ready.await(5, TimeUnit.SECONDS));
        } finally {
            MeronCoreNative.removeCoreEventListener(listener);
        }

        assertFalse(eventJson.get().isEmpty());
        JSONObject envelope = new JSONObject(eventJson.get());
        assertEquals("ready", envelope.getString("event"));
        assertEquals(1, envelope.getJSONObject("detail").getInt("protocol"));
    }
}
