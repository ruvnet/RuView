//! Transaction support for graph database operations

use std::collections::HashMap;
use uuid::Uuid;

/// Transaction state
#[derive(Debug, Clone)]
pub enum TransactionState {
    Active,
    Committed,
    RolledBack,
}

/// Transaction metadata
#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub state: TransactionState,
    pub operations: Vec<String>,
}

/// Transaction manager
pub struct TransactionManager {
    transactions: HashMap<String, Transaction>,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
        }
    }

    /// Begin a new transaction
    pub fn begin(&mut self) -> String {
        let tx_id = Uuid::new_v4().to_string();
        let tx = Transaction {
            id: tx_id.clone(),
            state: TransactionState::Active,
            operations: Vec::new(),
        };
        self.transactions.insert(tx_id.clone(), tx);
        tx_id
    }

    /// Commit a transaction
    pub fn commit(&mut self, tx_id: &str) -> Result<(), String> {
        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| format!("Transaction not found: {}", tx_id))?;

        match tx.state {
            TransactionState::Active => {
                tx.state = TransactionState::Committed;
                Ok(())
            }
            TransactionState::Committed => Err("Transaction already committed".to_string()),
            TransactionState::RolledBack => Err("Transaction already rolled back".to_string()),
        }
    }

    /// Rollback a transaction
    pub fn rollback(&mut self, tx_id: &str) -> Result<(), String> {
        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| format!("Transaction not found: {}", tx_id))?;

        match tx.state {
            TransactionState::Active => {
                tx.state = TransactionState::RolledBack;
                Ok(())
            }
            TransactionState::Committed => Err("Cannot rollback committed transaction".to_string()),
            TransactionState::RolledBack => Err("Transaction already rolled back".to_string()),
        }
    }

    /// Add an operation to a transaction
    pub fn add_operation(&mut self, tx_id: &str, operation: String) -> Result<(), String> {
        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| format!("Transaction not found: {}", tx_id))?;

        match tx.state {
            TransactionState::Active => {
                tx.operations.push(operation);
                Ok(())
            }
            _ => Err("Transaction is not active".to_string()),
        }
    }

    /// Get transaction state
    pub fn get_state(&self, tx_id: &str) -> Option<TransactionState> {
        self.transactions.get(tx_id).map(|tx| tx.state.clone())
    }

    /// Clean up old transactions
    pub fn cleanup(&mut self) {
        self.transactions
            .retain(|_, tx| matches!(tx.state, TransactionState::Active));
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_lifecycle() {
        let mut tm = TransactionManager::new();

        // Begin transaction
        let tx_id = tm.begin();
        assert!(matches!(
            tm.get_state(&tx_id),
            Some(TransactionState::Active)
        ));

        // Add operation
        tm.add_operation(&tx_id, "CREATE NODE".to_string()).unwrap();

        // Commit
        tm.commit(&tx_id).unwrap();
        assert!(matches!(
            tm.get_state(&tx_id),
            Some(TransactionState::Committed)
        ));

        // Cannot commit again
        assert!(tm.commit(&tx_id).is_err());
    }

    #[test]
    fn test_transaction_rollback() {
        let mut tm = TransactionManager::new();

        let tx_id = tm.begin();
        tm.add_operation(&tx_id, "CREATE NODE".to_string()).unwrap();

        // Rollback
        tm.rollback(&tx_id).unwrap();
        assert!(matches!(
            tm.get_state(&tx_id),
            Some(TransactionState::RolledBack)
        ));

        // Cannot rollback again
        assert!(tm.rollback(&tx_id).is_err());
    }
}
