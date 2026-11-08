//! Every email the app sends, composed here, text only, and handed to
//! the `mailer`. The app's name is `APP_NAME`, to change in a fork.

use crate::mailer::Mail;

pub const APP_NAME: &str = "Starter Pack";

/// The code a sign-in or a sign-up is finished with.
pub fn sign_in_code(to: &str, code: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: format!("Your {APP_NAME} code"),
        text: format!(
            "Your code is {code}. It expires in ten minutes.\n\n\
             If you did not ask for it, ignore this email."
        ),
    }
}

/// To someone who signed up with the email of an account they already
/// have; the sign-up answered as if it were new, see `accounts::sign_up`.
pub fn already_signed_up(to: &str) -> Mail {
    Mail {
        to: to.to_string(),
        subject: "You already have an account".to_string(),
        text: format!(
            "Someone, likely you, tried to sign up for {APP_NAME} with this email. \
             You already have an account: sign in with your password.\n\n\
             If this was not you, ignore this email."
        ),
    }
}

/// The six-digit code an email carries, for a test reading it back.
pub fn code_in(mail: &Mail) -> Option<String> {
    let digits: Vec<char> = mail.text.chars().collect();
    digits
        .windows(6)
        .find(|window| window.iter().all(char::is_ascii_digit))
        .map(|window| window.iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mails_sign_in_code_carries_the_code_and_code_in_reads_it_back() {
        let mail = sign_in_code("alice@example.com", "042137");
        assert_eq!(mail.to, "alice@example.com");
        assert!(mail.subject.contains(APP_NAME));
        assert_eq!(code_in(&mail), Some("042137".to_string()));
        assert_eq!(code_in(&already_signed_up("alice@example.com")), None);
    }
}
