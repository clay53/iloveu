use rand::Rng;
use sha2::Sha256;
use sha2::Digest;

const SALT_LEN: usize = 16;

pub type SessionID = u64;
pub type HashedSessionID = sha2::digest::Output<Sha256>;

#[derive(Debug)]
pub struct SessionManager {
    sessions_salt: [u8; SALT_LEN],
    next_session_id: SessionID,
    valid_hashed_session_ids: Vec<HashedSessionID>,
}

impl SessionManager {
    pub fn new() -> Self {
        let mut salt = [0; 16];
        rand::thread_rng().fill(&mut salt);

        Self {
            sessions_salt: salt,
            next_session_id: 0,
            valid_hashed_session_ids: Vec::new(),
        }
    }

    pub fn validate_session(&self, hashed_session_id: &HashedSessionID) -> bool {
        self.valid_hashed_session_ids.contains(hashed_session_id)
    }

    pub fn new_session(&mut self) -> HashedSessionID {
        let mut hasher = Sha256::new();
        hasher.update(self.sessions_salt);
        hasher.update(self.next_session_id.to_be_bytes());

        let hashed_session_id = hasher.finalize();

        self.valid_hashed_session_ids.push(hashed_session_id);

        self.next_session_id += 1;

        return hashed_session_id

    }

    pub fn invalidate_session(&mut self, hashed_session_id: HashedSessionID) -> bool {
        for (i, valid_hashed_session_id) in self.valid_hashed_session_ids.iter().enumerate() {
            if hashed_session_id == *valid_hashed_session_id {
                self.valid_hashed_session_ids.remove(i);
                return true;
            }
        }
        false
    }
}