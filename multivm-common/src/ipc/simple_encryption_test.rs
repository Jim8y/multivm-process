#[cfg(test)]
mod simple_tests {
    use crate::{IpcCommand, IpcMessage, ProcessId};
    use chacha20poly1305::{
        aead::{Aead, NewAead},
        ChaCha20Poly1305, Key, Nonce,
    };

    #[test]
    fn test_basic_chacha20poly1305() {
        // Test basic ChaCha20Poly1305 encryption/decryption
        let key = Key::from_slice(b"an example very very secret key."); // 32 bytes
        let cipher = ChaCha20Poly1305::new(key);

        let nonce = Nonce::from_slice(b"unique nonce"); // 12 bytes
        let plaintext = b"plaintext message";

        // Encrypt
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).unwrap();

        // Decrypt
        let decrypted = cipher.decrypt(nonce, ciphertext.as_ref()).unwrap();

        assert_eq!(&decrypted[..], plaintext);
        println!("✅ Basic ChaCha20Poly1305 test passed!");
    }

    #[test]
    fn test_ipc_message_serialization() {
        // Test that IPC messages can be serialized/deserialized
        let message = IpcMessage::new(ProcessId::Main, ProcessId::Solana, IpcCommand::Ping);

        let serialized = bincode::serialize(&message).unwrap();
        let deserialized: IpcMessage = bincode::deserialize(&serialized).unwrap();

        assert_eq!(message.source, deserialized.source);
        assert_eq!(message.destination, deserialized.destination);
        println!("✅ IPC message serialization test passed!");
    }

    #[test]
    fn test_encryption_key_derivation() {
        use sha2::{Digest, Sha256};

        // Test deterministic key derivation
        let shared_secret = b"test-shared-secret";

        let mut hasher = Sha256::new();
        hasher.update(shared_secret);
        hasher.update(b"MultiVM-IPC-Encryption");
        hasher.update(32u32.to_be_bytes());

        let key = hasher.finalize();
        assert_eq!(key.len(), 32);

        // Verify it's deterministic
        let mut hasher2 = Sha256::new();
        hasher2.update(shared_secret);
        hasher2.update(b"MultiVM-IPC-Encryption");
        hasher2.update(32u32.to_be_bytes());

        let key2 = hasher2.finalize();
        assert_eq!(key, key2);

        println!("✅ Key derivation test passed!");
    }
}
