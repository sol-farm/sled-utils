pub trait DbKey {
    /// returns the key of value being inserted into the db
    fn key(&self) -> anyhow::Result<&[u8]>;
}

/// various trees and their keys for use with sled
#[derive(Debug, Clone, Copy)]
pub enum DbTrees<'a> {
    Custom(&'a str),
}

impl<'a> ToString for DbTrees<'a> {
    fn to_string(&self) -> String {
        self.str().to_string()
    }
}

impl<'a> DbTrees<'a> {
    pub fn str(&self) -> &str {
        match self {
            DbTrees::Custom(tree_key) => tree_key,
        }
    }
}
