//! an embedded database using the sled framework
//!
use borsh::{BorshDeserialize, BorshSerialize};
pub mod config;
pub mod types;
use anyhow::{anyhow, Result};
use config::DbOpts;
use sled::{IVec, Tree};
use std::sync::Arc;

use self::types::{DbKey, DbTrees};

/// Database is the main embedded database object using the
/// sled db
#[derive(Clone)]
pub struct Database {
    db: sled::Db,
}

/// DbTree is a wrapper around the sled::Tree type providing
/// convenience functions
#[derive(Clone)]
pub struct DbTree {
    pub tree: Tree,
}

/// DbBatch is a wrapper around the sled::Batch type providing
/// convenience functions
#[derive(Default, Clone)]
pub struct DbBatch {
    batch: sled::Batch,
    count: u64,
}

impl Database {
    /// returns a new sled database
    pub fn new(cfg: &DbOpts) -> Result<Arc<Self>> {
        let sled_config: sled::Config = cfg.into();
        let db = sled_config.open()?;
        drop(sled_config);
        Ok(Arc::new(Database { db }))
    }
    /// opens the given database tree
    pub fn open_tree(self: &Arc<Self>, tree: DbTrees) -> Result<Arc<DbTree>> {
        DbTree::open(&self.db, tree)
    }
    /// opens the given db tree, return a vector of (key, value)
    pub fn list_values(self: &Arc<Self>, tree: DbTrees) -> Result<Vec<(IVec, IVec)>> {
        let tree = self.open_tree(tree)?;
        Ok(tree
            .iter()
            .filter_map(|entry| {
                if let Ok((key, value)) = entry {
                    Some((key, value))
                } else {
                    None
                }
            })
            .collect())
    }
    /// flushes teh database
    pub fn flush(self: &Arc<Self>) -> Result<usize> {
        Ok(self.db.flush()?)
    }
    /// returns a clone of the inner database
    pub fn inner(self: &Arc<Self>) -> sled::Db {
        self.db.clone()
    }
    /// destroys all trees except the default tree
    pub fn destroy(self: &Arc<Self>) {
        const SLED_DEFAULT_TREE: &[u8] = b"__sled__default";
        self.db
            .tree_names()
            .iter()
            .filter(|tree_name| tree_name.as_ref().ne(SLED_DEFAULT_TREE))
            .for_each(|tree_name| {
                if let Err(err) = self.db.drop_tree(tree_name) {
                    log::error!("failed to drop tree {:?}: {:#?}", tree_name.as_ref(), err);
                }
            });
    }
}

impl DbTree {
    pub fn open(db: &sled::Db, tree: DbTrees) -> Result<Arc<Self>> {
        let tree = db.open_tree(tree.str())?;
        Ok(Arc::new(Self { tree }))
    }
    pub fn len(&self) -> usize {
        self.tree.len()
    }
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }
    pub fn iter(&self) -> sled::Iter {
        self.tree.iter()
    }
    pub fn contains_key<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<bool> {
        self.tree.contains_key(key)
    }
    pub fn flush(&self) -> sled::Result<usize> {
        self.tree.flush()
    }
    pub fn apply_batch(&self, batch: &mut DbBatch) -> sled::Result<()> {
        self.tree.apply_batch(batch.take_inner())
    }
    pub fn insert<T>(&self, value: &T) -> Result<Option<sled::IVec>>
    where
        T: BorshSerialize + DbKey,
    {
        Ok(self.tree.insert(
            value.key()?,
            match borsh::to_vec(value) {
                Ok(data) => data,
                Err(err) => return Err(anyhow!("failed to insert entry {:#?}", err)),
            },
        )?)
    }
    pub fn get<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<Option<sled::IVec>> {
        self.tree.get(key)
    }
    pub fn deserialize<K: AsRef<[u8]>, T>(&self, key: K) -> Result<T>
    where
        T: BorshDeserialize,
    {
        let value = self.get(key)?;
        if let Some(value) = value {
            Ok(borsh::de::BorshDeserialize::try_from_slice(&value)?)
        } else {
            Err(anyhow!("value for key is None"))
        }
    }
}

impl DbBatch {
    pub fn new() -> DbBatch {
        DbBatch {
            batch: Default::default(),
            count: 0,
        }
    }
    pub fn insert<T>(&mut self, value: &T) -> Result<()>
    where
        T: BorshSerialize + DbKey,
    {
        self.batch.insert(
            value.key()?,
            match borsh::to_vec(value) {
                Ok(data) => data,
                Err(err) => return Err(anyhow!("failed to insert entry into batch {:#?}", err)),
            },
        );
        self.count += 1;
        Ok(())
    }
    /// returns the inner batch, and should only be used when the batch object
    /// is finished with and the batch needs to be applied, as it replaces the inner
    /// batch with its default version
    pub fn take_inner(&mut self) -> sled::Batch {
        std::mem::take(&mut self.batch)
    }
    pub fn inner(&self) -> &sled::Batch {
        &self.batch
    }
    pub fn count(&self) -> u64 {
        self.count
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
    use config::DbOpts;
    use std::fs::remove_dir_all;

    #[derive(BorshSerialize, BorshDeserialize, BorshSchema)]
    pub struct TestData {
        pub key: String,
        pub foo: String,
    }

    impl DbKey for TestData {
        fn key(&self) -> anyhow::Result<&[u8]> {
            Ok(self.key.as_bytes())
        }
    }

    // performs very basic database testing
    #[test]
    fn test_db_basic() {
        let db_opts = DbOpts::default();

        let db = Database::new(&db_opts).unwrap();
        let insert = || {
            let mut db_batch = DbBatch::new();
            db_batch
                .insert(&TestData {
                    key: "key1".to_string(),
                    foo: "foo1".to_string(),
                })
                .unwrap();
            {
                let tree = db.open_tree(DbTrees::Custom("foobar")).unwrap();
                tree.apply_batch(&mut db_batch).unwrap();
                assert_eq!(tree.len(), 1);
            }

            db_batch
                .insert(&TestData {
                    key: "key2".to_string(),
                    foo: "foo2".to_string(),
                })
                .unwrap();
            {
                let tree = db.open_tree(DbTrees::Custom("foobar")).unwrap();
                tree.apply_batch(&mut db_batch).unwrap();
                assert_eq!(tree.len(), 2);
            }

            db_batch
                .insert(&TestData {
                    key: "key3".to_string(),
                    foo: "foo3".to_string(),
                })
                .unwrap();
            {
                let tree = db.open_tree(DbTrees::Custom("foobarbaz")).unwrap();
                tree.apply_batch(&mut db_batch).unwrap();
                assert_eq!(tree.len(), 1);
            }
        };
        let query = || {
            let foobar_values = db.list_values(DbTrees::Custom("foobar")).unwrap();
            assert_eq!(foobar_values.len(), 2);
            let test_data_one: TestData = db
                .open_tree(DbTrees::Custom("foobar"))
                .unwrap()
                .deserialize(foobar_values[0].0.clone())
                .unwrap();
            assert_eq!(test_data_one.key, "key1".to_string());
            assert_eq!(test_data_one.foo, "foo1".to_string());
            let test_data_two: TestData = db
                .open_tree(DbTrees::Custom("foobar"))
                .unwrap()
                .deserialize(foobar_values[1].0.clone())
                .unwrap();
            assert_eq!(test_data_two.key, "key2".to_string());
            assert_eq!(test_data_two.foo, "foo2".to_string());
            let foobarbaz_values = db.list_values(DbTrees::Custom("foobarbaz")).unwrap();
            assert_eq!(foobarbaz_values.len(), 1);
            let test_data_three: TestData = db
                .open_tree(DbTrees::Custom("foobarbaz"))
                .unwrap()
                .deserialize(foobarbaz_values[0].0.clone())
                .unwrap();
            assert_eq!(test_data_three.key, "key3".to_string());
            assert_eq!(test_data_three.foo, "foo3".to_string());
        };
        insert();
        query();
        db.destroy();
        remove_dir_all("test_infos.db").unwrap();
    }
}
