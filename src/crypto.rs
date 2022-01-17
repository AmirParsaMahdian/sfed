use crate::file::{read_file, write_file};
use aes_gcm_siv::aead::{Aead, NewAead};
use aes_gcm_siv::{Aes256GcmSiv, Key, Nonce};
use anyhow::{anyhow, Result};
use pbkdf2::{password_hash::PasswordHasher, Pbkdf2};
use rand::{distributions::Alphanumeric, Rng};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 12;

pub struct Crypto {
    pub password: String,
    pub key_path: std::path::PathBuf,
}

impl Crypto {
    pub fn create_key(&self) -> Result<()> {
        println!("Creating key...");
        let pbkdf2_salt = Crypto::new_pbkdf2_salt();
        let pbkdf2_key = Crypto::generate_pbkdf2_key(self.password.as_bytes(), &pbkdf2_salt)?;
        let cipher = Crypto::generate_aes_cipher(&pbkdf2_key);

        let aes_nonce = Crypto::new_aes_nonce();
        let aes_key = Crypto::new_aes_key();

        let encrypted_key = Crypto::encrypt(&aes_key, &cipher, &aes_nonce)?;

        let key_content =
            Crypto::create_key_content(pbkdf2_salt.as_bytes(), &aes_nonce, &encrypted_key);

        write_file(&self.key_path, key_content, true)?;
        Ok(println!("Created key at {}.", self.key_path.display()))
    }

    pub fn change_password(&mut self, new_password: String) -> Result<()> {
        let aes_key = self.load_aes_key()?;
        let aes_nonce = self.load_aes_nonce()?;
        self.password = new_password;

        let pbkdf2_salt = Crypto::new_pbkdf2_salt();
        let pbkdf2_key = Crypto::generate_pbkdf2_key(self.password.as_bytes(), &pbkdf2_salt)?;
        let cipher = Crypto::generate_aes_cipher(&pbkdf2_key);

        let encrypted_key = Crypto::encrypt(&aes_key, &cipher, &aes_nonce)?;

        let key_content =
            Crypto::create_key_content(pbkdf2_salt.as_bytes(), &aes_nonce, &encrypted_key);

        write_file(&self.key_path, key_content, false)?;
        Ok(println!(
            "Changed the password of key at {}.",
            self.key_path.display()
        ))
    }

    pub fn encrypt_file(
        &self,
        input: std::path::PathBuf,
        output: std::path::PathBuf,
    ) -> Result<()> {
        let nonce = self.load_aes_nonce()?;
        let cipher = self.load_aes_cipher()?;
        let content = read_file(&input)?;
        let cipher_text = Crypto::encrypt(&content, &cipher, &nonce)?;
        write_file(&output, cipher_text, true)?;
        Ok(println!("File encrypted at {}.", output.display()))
    }

    pub fn decrypt_file(
        &self,
        input: std::path::PathBuf,
        output: std::path::PathBuf,
    ) -> Result<()> {
        let nonce = self.load_aes_nonce()?;
        let cipher = self.load_aes_cipher()?;
        let content = read_file(&input)?;
        let plain_text = Crypto::decrypt(&content, &cipher, &nonce)?;
        write_file(&output, plain_text, true)?;
        Ok(println!("File decrypted at {}.", output.display()))
    }
    
    fn create_key_content(pbkdf2_salt: &[u8], aes_nonce: &[u8], encrypted_key: &[u8]) -> Vec<u8> {
        let mut key_content = Vec::new();
        key_content.extend(pbkdf2_salt);
        key_content.push(b'\n');
        key_content.extend(aes_nonce);
        key_content.push(b'\n');
        key_content.extend(encrypted_key);
        key_content.push(b'\n');
        key_content
    }

    fn encrypt(input: &Vec<u8>, cipher: &Aes256GcmSiv, nonce: &Nonce) -> Result<Vec<u8>> {
        match cipher.encrypt(nonce, input.as_ref()) {
            Ok(plain_text) => Ok(plain_text),
            Err(_) => Err(anyhow!("Encryption failure!.")),
        }
    }

    fn decrypt(input: &Vec<u8>, cipher: &Aes256GcmSiv, nonce: &Nonce) -> Result<Vec<u8>> {
        match cipher.decrypt(nonce, input.as_ref()) {
            Ok(plain_text) => Ok(plain_text),
            Err(_) => Err(anyhow!("Decryption failure!.")),
        }
    }

    fn new_pbkdf2_salt() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(SALT_LEN)
            .map(char::from)
            .collect()
    }

    fn load_pbkdf2_salt(&self) -> Result<String> {
        let key_content = read_file(&self.key_path)?;
        Ok(String::from_utf8(key_content[0..SALT_LEN].to_vec())?)
    }

    fn load_pbkdf2_key(&self) -> Result<Vec<u8>> {
        let salt = self.load_pbkdf2_salt()?;
        Crypto::generate_pbkdf2_key(self.password.as_bytes(), &salt)
    }

    fn generate_pbkdf2_key(password: &[u8], salt: &String) -> Result<Vec<u8>> {
        match Pbkdf2.hash_password(password, &salt) {
            Ok(hash) => Ok(hash.hash.unwrap().as_bytes()[0..KEY_LEN].to_vec()),
            Err(_) => Err(anyhow!("PBKDF2 hash error.")),
        }
    }

    fn new_aes_nonce() -> Nonce {
        let nonce: [u8; NONCE_LEN] = rand::random();
        *Nonce::from_slice(&nonce)
    }

    fn new_aes_key() -> Vec<u8> {
        let key: [u8; KEY_LEN] = rand::random();
        key.to_vec()
    }

    fn read_aes_nonce(&self) -> Result<Vec<u8>> {
        let key_content = read_file(&self.key_path)?;
        Ok(key_content[SALT_LEN + 1..SALT_LEN + NONCE_LEN + 1].to_vec())
    }

    fn read_aes_key(&self) -> Result<Vec<u8>> {
        let key_content = read_file(&self.key_path)?;
        Ok(key_content[SALT_LEN + NONCE_LEN + 2..key_content.len() - 1].to_vec())
    }

    fn load_aes_nonce(&self) -> Result<Nonce> {
        Ok(*Nonce::from_slice(&self.read_aes_nonce()?))
    }

    fn load_aes_key(&self) -> Result<Vec<u8>> {
        let nonce = self.load_aes_nonce()?;
        let key = self.read_aes_key()?;
        let cipher = Crypto::generate_aes_cipher(&self.load_pbkdf2_key()?);
        Crypto::decrypt(&key, &cipher, &nonce)
    }

    fn load_aes_cipher(&self) -> Result<Aes256GcmSiv> {
        let key = match self.load_aes_key() {
            Ok(key) => key,
            Err(_) => return Err(anyhow!("Key authentication failure.")),
        };
        println!("Key authenticated.");
        Ok(Crypto::generate_aes_cipher(&key))
    }

    fn generate_aes_cipher(key: &Vec<u8>) -> Aes256GcmSiv {
        let key = Key::from_slice(&key);
        Aes256GcmSiv::new(key)
    }
}
