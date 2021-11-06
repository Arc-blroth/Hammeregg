use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rsa::RsaPrivateKey;
use serde::Serialize;
use zeroize::Zeroize;

/// Bit size for public/private keys
const BIT_SIZE: usize = 4096;

/// Generates two unrelated private keys.
pub fn gen_home_and_remote_keys() -> (RsaPrivateKey, RsaPrivateKey) {
    // init key-pairs
    let mut rng = ChaCha20Rng::from_entropy();
    let home_private_key = RsaPrivateKey::new(&mut rng, BIT_SIZE).unwrap();
    let mut rng = ChaCha20Rng::from_entropy();
    let remote_private_key = RsaPrivateKey::new(&mut rng, BIT_SIZE).unwrap();
    (home_private_key, remote_private_key)
}

/// The home public key and remote private key combo
/// that serves as Hammeregg's password.
///
/// Both keys are stored as PEM strings in this struct.
// not Deserialize since this is never read by Rust code
#[derive(Serialize, Zeroize)]
#[zeroize(drop)]
pub struct RemotePassword {
    pub home_public_key: String,
    pub remote_private_key: String,
}
