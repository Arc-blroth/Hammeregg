use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rsa::RsaPrivateKey;

/// Bit size for public/private keys
const BIT_SIZE: usize = 4096;

pub fn gen_home_and_remote_keys() -> (RsaPrivateKey, RsaPrivateKey) {
    // init key-pairs
    let mut rng = ChaCha20Rng::from_entropy();
    let home_private_key = RsaPrivateKey::new(&mut rng, BIT_SIZE).unwrap();
    let remote_private_key = RsaPrivateKey::new(&mut rng, BIT_SIZE).unwrap();
    (home_private_key, remote_private_key)
}
