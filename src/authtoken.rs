// For reference on JWT specification for APNs: https://developer.apple.com/documentation/usernotifications/setting_up_a_remote_notification_server/establishing_a_token-based_connection_to_apns
// also check out Apple's JSON Web Token Validator tool on their Push Notifications dashboard

use std::{fs, time::{SystemTime, UNIX_EPOCH}, env};
use openssl::{pkey::PKey, sign::Signer, hash::MessageDigest};
use base64::{engine::general_purpose, Engine};

use crate::util::{VAR_AUTH_KEY_ID, VAR_TOKEN_KEY_PATH, VAR_TEAM_ID};

pub struct AuthToken {
    pub token: String,
    jwt_header: String,
    jwt_claims: String,
    jwt_signed: String,
}

impl AuthToken {
    pub fn new() -> AuthToken {
        let jwt_header = AuthToken::generate_jwt_header();
        let jwt_claims = AuthToken::generate_jwt_claims();
        let jwt_signed = AuthToken::generate_jwt_signed(&jwt_header, &jwt_claims);

        match jwt_signed {
            Ok(jwt_signed) => {
                AuthToken {
                    token: AuthToken::get_formatted_token(&jwt_header, &jwt_claims, &jwt_signed),
                    jwt_header,
                    jwt_claims,
                    jwt_signed,
                }
            },
            Err(e) => {
                println!("Failed to generate authentication token {:?}", e);
                if let AuthTokenError::IO(_) = e {
                    println!("Failed to read token key path. If using Docker, ensure the private key is made available to a mounted volume.")
                }
                panic!();
            }
        }
    }

    pub fn refresh(&mut self) -> Result<(), AuthTokenError> {
        self.jwt_claims = AuthToken::generate_jwt_claims();
        self.jwt_signed = AuthToken::generate_jwt_signed(&self.jwt_header, &self.jwt_claims)?;
        self.token = AuthToken::get_formatted_token(&self.jwt_header, &self.jwt_claims, &self.jwt_signed);
        Ok(())
    }

    fn get_formatted_token(jwt_header: &String, jwt_claims: &String, jwt_signed: &String) -> String {
        format!("{jwt_header}.{jwt_claims}.{jwt_signed}")
    }

    fn generate_jwt_header() -> String {
        general_purpose::STANDARD_NO_PAD.encode(
            format!("{{ \"alg\": \"ES256\", \"kid\": \"{}\" }}", env::var(VAR_AUTH_KEY_ID).unwrap())
                .as_bytes()
        )
    }

    fn generate_jwt_claims() -> String {
        let now = SystemTime::now();
        let since_epoch = now.duration_since(UNIX_EPOCH).unwrap().as_secs();
        general_purpose::STANDARD_NO_PAD.encode(
            format!("{{ \"iss\": \"{}\", \"iat\": {} }}", env::var(VAR_TEAM_ID).unwrap(), since_epoch)
                .as_bytes()
        )
    }

    /// Signing using ECDSA
    fn generate_jwt_signed(header: &String, claims: &String) -> Result<String, AuthTokenError> {
        let header_claims = format!("{header}.{claims}");

        let private_key_bytes = fs::read(env::var(VAR_TOKEN_KEY_PATH).unwrap()).map_err(|e| AuthTokenError::IO(e))?;
        let key = PKey::private_key_from_pem(&private_key_bytes).map_err(|_| AuthTokenError::BadPrivateKey)?;
        
        let mut signer = Signer::new(MessageDigest::sha256(), &key).map_err(|_| AuthTokenError::BadSignature)?;

        signer.update(header_claims.as_bytes()).map_err(|_| AuthTokenError::BadSignature)?;
        let signed = signer.sign_to_vec().map_err(|_| AuthTokenError::BadSignature)?;

        let signed_encoded = general_purpose::STANDARD_NO_PAD.encode(signed);
        Ok(signed_encoded)
    }
}

#[derive(Debug)]
pub enum AuthTokenError {
    IO(std::io::Error),
    BadPrivateKey,
    BadSignature,
}
