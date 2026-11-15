//! Finds the one-time code in a "here is your sign-in code" mail, so clients can
//! offer it for copying from the notification and the message itself.
//!
//! The hard part is not finding a number, it is leaving the wrong ones alone:
//! receipts, orders, tickets and tracking mails are full of digit runs. A
//! candidate therefore has to be vouched for by the words around it. A number
//! counts only when the mail talks about a code where the number stands, and
//! nothing closer says it is an order, a receipt, an amount, a date or a phone
//! number. Deciding in the core keeps every client in agreement.

/// How much of the body is searched. The code of a real verification mail is
/// near the top; anything further down is footer, legal text or a quoted reply.
const BODY_LIMIT: usize = 4000;

/// How far before a candidate a "code" word still speaks for it, in characters.
const BEFORE_WINDOW: usize = 90;
/// "123456 is your verification code": the same, after the candidate.
const AFTER_WINDOW: usize = 60;
/// A word that disowns a candidate has to be this close in front of it.
const DISOWN_WINDOW: usize = 28;

/// Words that mean "this mail carries a one-time code". Lowercase; matched as
/// whole words where the script has word boundaries. Each is unambiguous on its
/// own, unlike the bare "code" handled by [`GENERIC`].
///
/// A trailing `*` marks a stem: the word may carry an ending. Turkish, Estonian
/// and Latvian attach "your", case and number to the noun ("doğrulama
/// kodunuz", "kinnituskoodi"), so the dictionary form alone would miss the
/// most common wordings.
const STRONG: &[&str] = &[
    // English
    "verification code",
    "verification pin",
    "verification number",
    "security code",
    "confirmation code",
    "authentication code",
    "authorization code",
    "authorisation code",
    "login code",
    "log-in code",
    "log in code",
    "sign-in code",
    "sign in code",
    "signin code",
    "sign-up code",
    "access code",
    "one-time code",
    "one time code",
    "one-time password",
    "one time password",
    "one-time passcode",
    "one-time pin",
    "single-use code",
    "temporary code",
    "passcode",
    "guard code",
    "otp",
    "2fa code",
    "two-factor code",
    "two-step verification",
    "2-step verification",
    "verify your email",
    "verify your identity",
    "magic code",
    "reset code",
    "recovery code",
    // Russian, Ukrainian
    "код подтверждения",
    "код проверки",
    "проверочный код",
    "код для входа",
    "код входа",
    "код авторизации",
    "код безопасности",
    "код доступа",
    "код верификации",
    "одноразовый код",
    "одноразовый пароль",
    "код активации",
    "код восстановления",
    "код для подтверждения",
    "код підтвердження",
    "код для входу",
    "одноразовий код",
    "одноразовий пароль",
    // Spanish, Portuguese, Italian, French
    "código de verificación",
    "código de seguridad",
    "código de confirmación",
    "código de acceso",
    "código de inicio de sesión",
    "código de verificação",
    "código de segurança",
    "código de confirmação",
    "código de acesso",
    "codice di verifica",
    "codice di sicurezza",
    "codice di conferma",
    "codice di accesso",
    "code de vérification",
    "code de sécurité",
    "code de confirmation",
    "code de connexion",
    "code d'accès",
    "code d’accès",
    // German, Dutch, Swedish, Polish, Turkish, Indonesian, Vietnamese
    "bestätigungscode",
    "sicherheitscode",
    "verifizierungscode",
    "anmeldecode",
    "einmalcode",
    "einmalpasswort",
    "zugangscode",
    "verificatiecode",
    "beveiligingscode",
    "bevestigingscode",
    "inlogcode",
    "verifieringskod",
    "säkerhetskod",
    "engångskod",
    "kod weryfikacyjn*",
    "kodu weryfikacyjn*",
    "kodem weryfikacyjn*",
    "kod potwierdzając*",
    "kodu potwierdzając*",
    "kod bezpieczeństwa",
    "kod logowania",
    "hasło jednorazowe",
    "doğrulama kodu*",
    "güvenlik kodu*",
    "onay kodu*",
    "giriş kodu*",
    "tek kullanımlık*",
    "kode verifikasi*",
    "kode keamanan",
    "kode otp",
    "mã xác minh",
    "mã xác nhận",
    "mã bảo mật",
    // Chinese, Japanese, Korean: no spaces, so these match as substrings.
    "验证码",
    "驗證碼",
    "校验码",
    "动态码",
    "安全码",
    "確認コード",
    "認証コード",
    "認証番号",
    "確認番号",
    "ワンタイムパスワード",
    "セキュリティコード",
    "인증번호",
    "인증 번호",
    "인증코드",
    "인증 코드",
    "보안 코드",
    "확인 코드",
    // Arabic, Greek, Estonian, Latvian
    "رمز التحقق",
    "رمز التأكيد",
    "رمز الأمان",
    "κωδικός επαλήθευσης",
    "κωδικός επιβεβαίωσης",
    "κωδικός ασφαλείας",
    "kinnituskood*",
    "turvakood*",
    "verifitseerimiskood*",
    "ühekordne kood*",
    "ühekordse kood*",
    "apstiprinājuma kod*",
    "verifikācijas kod*",
    "drošības kod*",
];

/// The bare word for "code". It only counts when the mail is about signing in
/// or confirming something (see [`TOPIC`]) and the word in front of it does not
/// make it a different kind of code (see [`OTHER_CODES`]).
const GENERIC: &[&str] = &[
    "code",
    "pin",
    "код",
    "пароль",
    "código",
    "codigo",
    "codice",
    "kod",
    "kode",
    "kodu*",
    "mã",
    "コード",
    "코드",
    "κωδικός",
    "kood*",
    "kod*",
    "رمز",
];

/// What a mail with a bare "code" has to be about for the code to be a
/// one-time one.
const TOPIC: &[&str] = &[
    "verif",
    "confirm",
    "sign in",
    "sign-in",
    "signin",
    "signing in",
    "log in",
    "log-in",
    "login",
    "logging in",
    "authenticat",
    "two-factor",
    "2fa",
    "one-time",
    "one time",
    "expire",
    "valid for",
    "do not share",
    "don't share",
    "don’t share",
    "never share",
    "password reset",
    "reset your password",
    "подтвержд",
    "вход",
    "войти",
    "авториз",
    "никому не сообщайте",
    "не сообщайте",
    "действителен",
    "действует",
    "истекает",
    "сброс пароля",
    "підтвердж",
    "вхід",
    "увійти",
    "verific",
    "confirma",
    "iniciar sesión",
    "inicio de sesión",
    "caduca",
    "expira",
    "connexion",
    "vérifi",
    "bestätig",
    "anmeld",
    "gültig",
    "läuft ab",
    "inloggen",
    "bevestig",
    "logowani",
    "potwierd",
    "doğrula",
    "giriş",
    "verifikasi",
    "xác minh",
    "đăng nhập",
    "登录",
    "登入",
    "验证",
    "驗證",
    "ログイン",
    "認証",
    "로그인",
    "인증",
];

/// Words that, right in front of a bare "code", name some other kind of code.
const OTHER_CODES: &[&str] = &[
    "promo",
    "promotional",
    "discount",
    "coupon",
    "voucher",
    "gift",
    "referral",
    "invite",
    "invitation",
    "zip",
    "postal",
    "post",
    "area",
    "country",
    "dial",
    "error",
    "status",
    "source",
    "qr",
    "bar",
    "tracking",
    "order",
    "product",
    "item",
    "booking",
    "reservation",
    "offer",
    "sort",
    "swift",
    "bank",
    "card",
    "tax",
    "color",
    "colour",
    "dress",
    "промо",
    "почтовый",
    "скидочный",
    "подарочный",
    "реферальный",
    "штрих",
    "qr-",
    "трек",
    "descuento",
    "promocional",
    "postal",
    "réduction",
    "rabatt",
    "gutschein",
    "kortings",
    "rabatowy",
    "indirim",
];

/// Words right after a candidate that make it a code to spend, not to sign in
/// with: "Use code 482913 at checkout for 25% off".
const SPENT_AFTER: &[&str] = &[
    "at checkout",
    "at the checkout",
    "checkout",
    "discount",
    "% off",
    "off your",
    "coupon",
    "promo",
    "voucher",
    "при оформлении",
    "скидк",
    "промокод",
    "знижк",
    "descuento",
    "desconto",
    "réduction",
    "rabatt",
    "sconto",
    "korting",
    "zniżk",
    "rabat",
    "indirim",
];

/// Words that claim the number after them for something else. Checked in the
/// few characters in front of a candidate, where they beat any "code" word
/// further away: "Your code is below. Order 482913" is an order number.
const DISOWN: &[&str] = &[
    "order",
    "receipt",
    "invoice",
    "check",
    "cheque",
    "bill",
    "ticket",
    "booking",
    "reservation",
    "reference",
    "ref",
    "tracking",
    "shipment",
    "parcel",
    "package",
    "transaction",
    "payment",
    "amount",
    "total",
    "balance",
    "price",
    "account",
    "customer",
    "client",
    "member",
    "user id",
    "case",
    "request",
    "confirmation number",
    "phone",
    "tel",
    "fax",
    "call",
    "sms to",
    "text to",
    "zip",
    "postal",
    "po box",
    "version",
    "build",
    "copyright",
    "id",
    "no",
    "nr",
    "number",
    "ending in",
    "ending with",
    "card",
    "чек",
    "чека",
    "чеку",
    "заказ",
    "заказа",
    "заказу",
    "счёт",
    "счет",
    "счёта",
    "счета",
    "квитанция",
    "квитанции",
    "накладная",
    "накладной",
    "отправление",
    "отправления",
    "посылка",
    "посылки",
    "трек",
    "трек-номер",
    "билет",
    "билета",
    "бронь",
    "брони",
    "бронирование",
    "бронирования",
    "заявка",
    "заявки",
    "обращение",
    "обращения",
    "договор",
    "договора",
    "транзакция",
    "транзакции",
    "операция",
    "операции",
    "платёж",
    "платеж",
    "платежа",
    "сумма",
    "сумму",
    "итого",
    "баланс",
    "клиент",
    "клиента",
    "лицевой",
    "номер",
    "номера",
    "тел",
    "телефон",
    "телефона",
    "карта",
    "карты",
    "карте",
    "версия",
    "индекс",
    "замовлення",
    "рахунок",
    "pedido",
    "factura",
    "recibo",
    "fatura",
    "commande",
    "facture",
    "reçu",
    "bestellung",
    "rechnung",
    "beleg",
    "bestelling",
    "factuur",
    "zamówienie",
    "faktura",
    "paragon",
    "sipariş",
    "fatura",
    "订单",
    "訂單",
    "注文",
    "주문",
];

/// The one-time code a mail carries, or `None`.
///
/// `body` is the message's plain text. A code spelled with a separator ("123
/// 456", "123-456") comes back without it, ready to paste.
pub fn detect(subject: &str, body: &str) -> Option<String> {
    let text = searched_text(subject, body);
    // One character in, one out, so the n-th character of `lower` is the n-th
    // of `text` and a candidate found in one can be read from the other.
    let lower: String = text.chars().map(lowercase_char).collect();
    let chars: Vec<(usize, char)> = lower.char_indices().collect();
    let mail_is_about_codes = contains_any(&lower, TOPIC) || has_keyword(&lower, STRONG);

    let mut best: Option<(i32, usize, String)> = None;
    for candidate in candidates(&text, &chars) {
        let Some(score) = score(&lower, &chars, &candidate, mail_is_about_codes) else {
            continue;
        };
        // Highest score wins; the earlier candidate on a tie, since the code
        // comes before the "expires in 10 minutes" that follows it.
        if best.as_ref().is_none_or(|(top, _, _)| score > *top) {
            best = Some((score, candidate.start, candidate.code));
        }
    }
    best.map(|(_, _, code)| code)
}

/// Lowercases a single character. `str::to_lowercase` may turn one character
/// into two (Turkish `İ` becomes `i` plus a combining dot), which would put
/// every later character at a different index; a character whose lowercase is
/// not a single character is kept as it is. `İ` is mapped to `i` by hand, since
/// Turkish mail is full of it ("GİRİŞ", "İşlem") and the keywords are spelled
/// with a plain `i`.
fn lowercase_char(c: char) -> char {
    if c == 'İ' {
        return 'i';
    }
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ => c,
    }
}

/// Subject and the top of the body, as one text. The subject goes first: "Your
/// code is 123456" is often all there is.
fn searched_text(subject: &str, body: &str) -> String {
    let mut end = body.len().min(BODY_LIMIT);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n\n{}", subject.trim(), &body[..end])
}

struct Candidate {
    /// Index into the character list of the first and one-past-last character.
    start: usize,
    end: usize,
    code: String,
    /// Letters and digits mixed, which needs more convincing than digits.
    mixed: bool,
}

/// Every run of characters shaped like a code: 4 to 8 digits, two or three
/// digit groups joined by a space or hyphen, or a short run mixing capitals
/// and digits.
fn candidates(text: &str, chars: &[(usize, char)]) -> Vec<Candidate> {
    // Case matters for the mixed kind, so the shape is read from the original.
    let original: Vec<char> = text.chars().collect();
    debug_assert_eq!(original.len(), chars.len());
    let mut found = Vec::new();
    let mut at = 0;
    while at < original.len() {
        if !original[at].is_alphanumeric() {
            at += 1;
            continue;
        }
        let mut end = at;
        while end < original.len() && original[end].is_alphanumeric() {
            end += 1;
        }
        let run = &original[at..end];
        if run.iter().all(char::is_ascii_digit) {
            // Join "123 456" / "123-456" / "12 34 56": equal groups of 2 to 4.
            let mut groups = vec![(at, end)];
            let mut next = end;
            while groups.len() < 4
                && next + 1 < original.len()
                && (original[next] == ' ' || original[next] == '-')
                && original[next + 1].is_ascii_digit()
            {
                let mut group_end = next + 1;
                while group_end < original.len() && original[group_end].is_ascii_digit() {
                    group_end += 1;
                }
                let size = group_end - (next + 1);
                let touches_word =
                    group_end < original.len() && original[group_end].is_alphanumeric();
                if size != end - at || !(2..=4).contains(&size) || touches_word {
                    break;
                }
                groups.push((next + 1, group_end));
                next = group_end;
            }
            let grouped: String = groups
                .iter()
                .flat_map(|(from, to)| original[*from..*to].iter())
                .collect();
            if groups.len() > 1 && (4..=9).contains(&grouped.len()) {
                found.push(Candidate {
                    start: at,
                    end: next,
                    code: grouped,
                    mixed: false,
                });
                at = next;
                continue;
            }
            if (4..=8).contains(&run.len()) {
                found.push(Candidate {
                    start: at,
                    end,
                    code: run.iter().collect(),
                    mixed: false,
                });
            }
        } else if (5..=10).contains(&run.len())
            && run
                .iter()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && run.iter().filter(|c| c.is_ascii_digit()).count() >= 2
            && run.iter().any(char::is_ascii_uppercase)
        {
            found.push(Candidate {
                start: at,
                end,
                code: run.iter().collect(),
                mixed: true,
            });
        }
        at = end;
    }
    found
}

/// How convincingly the surrounding text says the candidate is a one-time
/// code, or `None` when it does not, or says it is something else.
fn score(
    lower: &str,
    chars: &[(usize, char)],
    candidate: &Candidate,
    mail_is_about_codes: bool,
) -> Option<i32> {
    if looks_like_something_else(chars, candidate) {
        return None;
    }
    let byte = |index: usize| chars.get(index).map_or(lower.len(), |(offset, _)| *offset);
    let before = &lower[byte(candidate.start.saturating_sub(BEFORE_WINDOW))..byte(candidate.start)];
    let after = &lower[byte(candidate.end)..byte((candidate.end + AFTER_WINDOW).min(chars.len()))];
    // A code is usually set off from its sentence by a blank line, so one
    // blank line does not break the link. Two do: that is another section.
    let before = before.rsplit("\n\n\n").next().unwrap_or(before);
    let after_line = after.split('\n').next().unwrap_or(after);

    let near = &lower[byte(candidate.start.saturating_sub(DISOWN_WINDOW))..byte(candidate.start)];
    let near = near.rsplit('\n').next().unwrap_or(near);
    let disowned = last_keyword(near, DISOWN);

    let strong_before = last_keyword(before, STRONG);
    let strong_after = first_keyword(after_line, STRONG);
    // The bare word counts only in a mail about signing in or confirming.
    // "Use code SAVE2024 at checkout" has the word right against the code and
    // is still not a one-time code.
    let generic_before = last_generic(before).filter(|_| mail_is_about_codes);
    let generic_after = first_generic(after_line).filter(|_| mail_is_about_codes);
    // And even there, not when the code is one to spend.
    if contains_any(after_line, SPENT_AFTER) {
        return None;
    }

    // Distance from the vouching word to the candidate; smaller is better.
    let vouched = [
        strong_before.map(|at| (before.len() - at, 40, true)),
        strong_after.map(|at| (at, 36, true)),
        generic_before.map(|at| (before.len() - at, 24, false)),
        generic_after.map(|at| (at, 20, false)),
    ]
    .into_iter()
    .flatten()
    .min_by_key(|(distance, _, _)| *distance);
    let (distance, weight, strong) = vouched?;

    // A disowning word between the "code" word and the number wins. One that
    // comes before the "code" word was talking about something earlier.
    if let Some(at) = disowned {
        let disown_distance = near.len() - at;
        let vouched_before = strong_before.is_some() || generic_before.is_some();
        if !vouched_before || disown_distance <= distance {
            return None;
        }
    }

    let mut score = weight - (distance as i32) / 8;
    if stands_alone(chars, candidate) {
        score += 12;
    }
    if candidate.mixed {
        // "ABC123" is as likely a promo code, a product or a reference. Only
        // an unambiguous phrase vouches for one, and only when it is set apart
        // or sits right after the phrase.
        if !strong || (!stands_alone(chars, candidate) && distance > 24) {
            return None;
        }
        score -= 6;
    } else if candidate.code.len() == 6 {
        // By far the most common length.
        score += 4;
    }
    if candidate.code.len() == 4
        && looks_like_year(&candidate.code)
        && !stands_alone(chars, candidate)
    {
        return None;
    }
    Some(score)
}

/// Numbers whose own shape gives them away: money, percentages, dates, times,
/// phone numbers, parts of a longer identifier, a link or an address.
fn looks_like_something_else(chars: &[(usize, char)], candidate: &Candidate) -> bool {
    let at = |index: isize| -> Option<char> {
        usize::try_from(index)
            .ok()
            .and_then(|index| chars.get(index))
            .map(|(_, c)| *c)
    };
    let start = candidate.start as isize;
    let end = candidate.end as isize;
    let prev = at(start - 1);
    let next = at(end);
    let prev2 = at(start - 2);
    let next2 = at(end + 1);
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());

    // Glued to something on the left: #1234, №1234, $1234, +1234, /1234, =1234,
    // @1234, 12.3456, 12:3456, 12-3456, 12/3456.
    if let Some(c) = prev {
        if "#№$€£¥₽₴+/=@_&?%~^\\".contains(c) {
            return true;
        }
        if ".,:-".contains(c) && digit(prev2) {
            return true;
        }
    }
    // "№ 1234", "# 1234", "$ 1234", "+7 1234".
    if prev == Some(' ') && prev2.is_some_and(|c| "#№$€£¥₽₴+".contains(c)) {
        return true;
    }
    // Glued to something on the right: 1234.56, 1234,5, 1234:56, 1234-56,
    // 1234/56, 1234%, 1234@, 1234$ and currencies.
    if let Some(c) = next {
        if "%$€£¥₽₴@/=_&~^\\".contains(c) {
            return true;
        }
        if ".,:-".contains(c) && digit(next2) {
            return true;
        }
    }
    // "1234 ₽", "1234 руб", "1234 usd", "1234 kg" and the like.
    if next == Some(' ') {
        let tail: String = (0..5).filter_map(|offset| at(end + 1 + offset)).collect();
        const UNITS: &[&str] = &[
            "₽",
            "$",
            "€",
            "£",
            "¥",
            "₴",
            "руб",
            "р.",
            "грн",
            "usd",
            "eur",
            "gbp",
            "rub",
            "uah",
            "jpy",
            "cny",
            "kzt",
            "тг",
            "тенге",
            "kg",
            "кг",
            "km",
            "км",
            "mb",
            "gb",
            "мб",
            "гб",
            "шт",
            "pcs",
            "px",
            "ms",
            "мс",
        ];
        if UNITS.iter().any(|unit| tail.starts_with(unit)) {
            return true;
        }
    }
    false
}

/// On a line of its own, or set off by emphasis or brackets: how mails present
/// the one thing they want typed in.
fn stands_alone(chars: &[(usize, char)], candidate: &Candidate) -> bool {
    let decoration = |c: char| c.is_whitespace() || "*_`|>[](){}<>:\"'«»“”‘’".contains(c);
    let mut left = candidate.start;
    while left > 0 && chars[left - 1].1 != '\n' {
        left -= 1;
        if !decoration(chars[left].1) {
            return false;
        }
    }
    let mut right = candidate.end;
    while right < chars.len() && chars[right].1 != '\n' {
        if !decoration(chars[right].1) && !".!".contains(chars[right].1) {
            return false;
        }
        right += 1;
    }
    true
}

fn looks_like_year(code: &str) -> bool {
    code.parse::<u32>()
        .is_ok_and(|year| (1990..=2099).contains(&year))
}

fn contains_any(text: &str, words: &[&str]) -> bool {
    words.iter().any(|word| text.contains(word))
}

fn has_keyword(text: &str, words: &[&str]) -> bool {
    first_keyword(text, words).is_some()
}

/// Byte offset of the earliest match of any of `words`.
fn first_keyword(text: &str, words: &[&str]) -> Option<usize> {
    words
        .iter()
        .filter_map(|word| keyword_matches(text, word).next())
        .map(|(start, _)| start)
        .min()
}

/// Byte offset just past the latest match of any of `words`.
fn last_keyword(text: &str, words: &[&str]) -> Option<usize> {
    words
        .iter()
        .filter_map(|word| keyword_matches(text, word).last())
        .map(|(_, end)| end)
        .max()
}

fn first_generic(text: &str) -> Option<usize> {
    GENERIC
        .iter()
        .flat_map(|word| keyword_matches(text, word))
        .filter(|(start, _)| !names_another_code(text, *start))
        .map(|(start, _)| start)
        .min()
}

fn last_generic(text: &str) -> Option<usize> {
    GENERIC
        .iter()
        .flat_map(|word| keyword_matches(text, word))
        .filter(|(start, _)| !names_another_code(text, *start))
        .map(|(_, end)| end)
        .max()
}

/// Whether the word in front of the "code" at `at` makes it a promo code, a
/// postal code and so on.
fn names_another_code(text: &str, at: usize) -> bool {
    let head = text[..at].trim_end();
    let word_start = head
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '-'))
        .map_or(0, |(index, c)| index + c.len_utf8());
    let word = &head[word_start..];
    !word.is_empty()
        && OTHER_CODES
            .iter()
            .any(|other| word == *other || word.strip_suffix('-') == Some(other))
}

/// Where `word` occurs as a word of its own, as byte ranges. A letter or digit
/// against its start disqualifies a match, and so does one against its end
/// unless the word is a stem (a trailing `*`), whose range then runs to the end
/// of the ending. Scripts without spaces between words (CJK) match anywhere.
fn keyword_matches<'a>(text: &'a str, word: &'a str) -> impl Iterator<Item = (usize, usize)> + 'a {
    let (word, stem) = match word.strip_suffix('*') {
        Some(stem) => (stem, true),
        None => (word, false),
    };
    let spaced = !word.chars().any(is_unspaced_script);
    text.match_indices(word).filter_map(move |(at, _)| {
        let end = at + word.len();
        if !spaced {
            return Some((at, end));
        }
        if text[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
        {
            return None;
        }
        let ending: usize = text[end..]
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .map(char::len_utf8)
            .sum();
        (stem || ending == 0).then_some((at, end + ending))
    })
}

fn is_unspaced_script(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF)
}

#[cfg(test)]
mod tests {
    use super::detect;

    fn code(subject: &str, body: &str) -> Option<String> {
        detect(subject, body)
    }

    #[test]
    fn finds_codes_in_the_usual_wordings() {
        let cases = [
            (
                "Your verification code",
                "Your verification code is 482913.",
                "482913",
            ),
            ("482913 is your Acme verification code", "", "482913"),
            (
                "Sign in to Acme",
                "Enter this code to sign in:\n\n**731 904**\n\nIt expires in 10 minutes.",
                "731904",
            ),
            (
                "Confirm your email",
                "Use the code below to verify your email address.\n\n5521\n",
                "5521",
            ),
            (
                "Security alert",
                "Your one-time password (OTP) is 90817263. Do not share it with anyone.",
                "90817263",
            ),
            (
                "Код подтверждения",
                "Ваш код подтверждения: 394 221. Никому не сообщайте его.",
                "394221",
            ),
            (
                "Вход в аккаунт",
                "Код для входа — 660142. Действителен 5 минут.",
                "660142",
            ),
            (
                "Google verification",
                "G-284719 is your Google verification code.",
                "284719",
            ),
            (
                "Your login code",
                "Here is your login code:\n\nQ7K2M9\n\nIf you did not request it, ignore this email.",
                "Q7K2M9",
            ),
            ("验证码", "您的验证码是 558201，5 分钟内有效。", "558201"),
            ("認証コードのお知らせ", "認証コード: 104-882", "104882"),
            (
                "Tu código de verificación",
                "Tu código de verificación es 771203.",
                "771203",
            ),
            (
                "Ihr Bestätigungscode",
                "Ihr Bestätigungscode lautet 918273.",
                "918273",
            ),
        ];
        for (subject, body, want) in cases {
            assert_eq!(
                code(subject, body).as_deref(),
                Some(want),
                "{subject} / {body}"
            );
        }
    }

    #[test]
    fn leaves_receipts_orders_and_other_numbers_alone() {
        let cases = [
            (
                "Ваш чек",
                "Спасибо за покупку! Номер чека 482913. Сумма 1 290 ₽.",
            ),
            (
                "Чек № 004512",
                "Кассовый чек №004512 от 12.03.2026. Итого: 4590 руб.",
            ),
            (
                "Order confirmation",
                "Thanks for your order. Order number: 77120934. Total: $1299.00",
            ),
            (
                "Your receipt from Acme",
                "Receipt #2048-1177. Amount paid 4500 USD.",
            ),
            (
                "Заказ 558201 оформлен",
                "Ваш заказ 558201 принят и будет доставлен 14.03.2026.",
            ),
            (
                "Your parcel is on its way",
                "Tracking number 9400111899. Call 8005551234 with questions.",
            ),
            (
                "Invoice 2026-0418",
                "Invoice 20260418 is due on 2026-05-01. Account 55120098.",
            ),
            (
                "Weekly digest",
                "In 2025 we shipped 1200 changes across 4812 commits.",
            ),
            (
                "Save 20% today",
                "Use promo code SPRING2026 at checkout. Offer ends 31.03.2026.",
            ),
            (
                "Booking confirmed",
                "Booking reference 884213. Check-in 15:00, room 4021.",
            ),
            ("Meeting at 1430", "See you at 1430 in room 5521."),
            (
                "Password changed",
                "Your password was changed on 12.03.2026 at 14:05 from 192.168.1.20.",
            ),
        ];
        for (subject, body) in cases {
            assert_eq!(code(subject, body), None, "{subject} / {body}");
        }
    }

    #[test]
    fn tells_the_code_from_the_numbers_around_it() {
        let cases = [
            ("Sign in to Acme", "Your code is 123456", Some("123456")),
            // The bare word is not enough when the mail is not about signing in.
            ("Your code", "Your code is 123456", None),
            (
                "Confirm sign-in",
                "Your code: 123456. Order #99887766 is unaffected.",
                Some("123456"),
            ),
            (
                "Подтверждение операции",
                "Код 4821 для подтверждения операции на сумму 1 290 ₽.",
                Some("4821"),
            ),
            (
                "Покупка",
                "Покупка 1290 ₽ в магазине. Код: 4821. Никому не сообщайте его.",
                Some("4821"),
            ),
            (
                "Your Apple ID code",
                "Your Apple ID verification code is: 482913",
                Some("482913"),
            ),
            (
                "Sign in",
                "Your verification code is 482913.\n\n© 2026 Acme Inc., 1200 Main Street",
                Some("482913"),
            ),
            (
                "Verify",
                "[Verify my email](https://acme.example/verify?code=482913)",
                None,
            ),
            (
                "Ticket 482913",
                "Ticket 482913: we could not reproduce your sign-in code issue.",
                None,
            ),
            (
                "Your request",
                "Reference number: 482913\nUse this page to log in and see the status.",
                None,
            ),
            (
                "Error",
                "The build failed with error code 500123. See the log.",
                None,
            ),
            (
                "Доставка",
                "Код подтверждения придёт в SMS. Трек-номер 482913 уже активен.",
                None,
            ),
            (
                "Call us",
                "Verification code problems? Call +1 (800) 555-1234 or 88005553535.",
                None,
            ),
        ];
        for (subject, body, want) in cases {
            assert_eq!(code(subject, body).as_deref(), want, "{subject} / {body}");
        }
    }

    #[test]
    fn codes_to_spend_are_not_codes_to_sign_in_with() {
        let cases = [
            (
                "Summer sale",
                "Use the code SUMMER25 at checkout for 25% off.",
            ),
            ("Save today", "Use code SAVE2024 at checkout."),
            ("Your gift", "Your gift card code is 4821 9930"),
            (
                "Confirm your subscription",
                "Thanks for confirming! Use code 482913 at checkout for 10% off.",
            ),
            (
                "Verify your email",
                "You are verified. Your code WELCOME10 gives a discount on the first order.",
            ),
            (
                "Подтверждение подписки",
                "Спасибо за подписку! Код 482913 даёт скидку 10% на первый заказ.",
            ),
        ];
        for (subject, body) in cases {
            assert_eq!(code(subject, body), None, "{subject} / {body}");
        }
    }

    #[test]
    fn a_letters_and_digits_code_needs_an_unambiguous_phrase() {
        // "code" alone, in a mail about signing in: not enough for a mixed one.
        assert_eq!(
            code("Sign in", "Enter the code AB12CD to continue signing in."),
            None
        );
        assert_eq!(
            code("Sign in", "Your verification code is AB12CD.").as_deref(),
            Some("AB12CD")
        );
    }

    #[test]
    fn turkish_capital_i_does_not_switch_detection_off() {
        assert_eq!(
            code("Sign in", "Your verification code is 482913.\n\nİyi günler").as_deref(),
            Some("482913")
        );
        assert_eq!(
            code("GİRİŞ KODU", "GİRİŞ KODUNUZ: 482913").as_deref(),
            Some("482913")
        );
    }

    #[test]
    fn nouns_with_endings_are_recognised() {
        let cases = [
            ("Doğrulama", "Doğrulama kodunuz: 482913", "482913"),
            (
                "Hesabınız",
                "Güvenlik kodunuzu kimseyle paylaşmayın: 771203",
                "771203",
            ),
            (
                "Kinnitus",
                "Teie kinnituskoodi väärtus on 558201.",
                "558201",
            ),
            (
                "Apstiprinājums",
                "Jūsu apstiprinājuma kodu: 104882",
                "104882",
            ),
            (
                "Weryfikacja",
                "Wpisz 660142 jako kod weryfikacyjny.",
                "660142",
            ),
            (
                "Weryfikacja",
                "Twojego kodu weryfikacyjnego 918273 nie udostępniaj nikomu.",
                "918273",
            ),
        ];
        for (subject, body, want) in cases {
            assert_eq!(
                code(subject, body).as_deref(),
                Some(want),
                "{subject} / {body}"
            );
        }
    }

    #[test]
    fn a_stem_does_not_match_in_the_middle_of_a_word() {
        // "kod*" must start a word: "metodkod" is not a code word.
        assert_eq!(code("Rapor", "Yeni metodkodu 482913 yayında."), None);
    }

    #[test]
    fn a_code_mail_that_also_mentions_an_order_still_yields_the_code() {
        let body = "Order 77120934 needs confirmation.\n\nYour verification code is 482913.\n\nTotal: $49.00";
        assert_eq!(
            code("Confirm your purchase", body).as_deref(),
            Some("482913")
        );
    }

    #[test]
    fn a_receipt_that_mentions_codes_does_not_turn_its_numbers_into_one() {
        let body = "Use the code we sent by SMS to confirm.\n\n\nНомер чека: 482913\nСумма: 1290 ₽";
        assert_eq!(code("Подтверждение оплаты", body), None);
    }

    #[test]
    fn numbers_far_from_any_code_word_are_ignored() {
        let filler = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(4);
        let body =
            format!("Your verification code is on its way.\n\n{filler}\n\n482913 people use Acme.");
        assert_eq!(code("Welcome", &body), None);
    }

    #[test]
    fn codes_past_the_searched_part_of_the_body_are_ignored() {
        let body = format!("{}\nYour verification code is 482913.", "x ".repeat(3000));
        assert_eq!(code("Hello", &body), None);
    }
}
