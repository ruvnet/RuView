//! Integration tests for certificate system

use roaring::RoaringBitmap;
use ruvector_mincut::prelude::*;
use ruvector_mincut::{
    AuditData, AuditEntryType, AuditLogger, CertLocalKCutQuery, CertificateError, CutCertificate,
    LocalKCutResponse, LocalKCutResultSummary, UpdateTrigger, UpdateType,
};

#[test]
fn test_certificate_creation() {
    let cert = CutCertificate::new();
    assert_eq!(cert.num_witnesses(), 0);
    assert_eq!(cert.num_responses(), 0);
    assert!(cert.best_witness().is_none());
    assert!(cert.certified_value().is_none());
}

#[test]
fn test_certificate_with_witnesses() {
    let witness1 = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);
    let witness2 = WitnessHandle::new(2, RoaringBitmap::from_iter([2, 4, 5]), 3);

    let witnesses = vec![witness1, witness2];
    let cert = CutCertificate::with_witnesses(witnesses);

    assert_eq!(cert.num_witnesses(), 2);
}

#[test]
fn test_certificate_add_witness() {
    let mut cert = CutCertificate::new();
    let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);

    cert.set_best_witness(0, witness.clone());

    assert_eq!(cert.num_witnesses(), 1);
    assert_eq!(cert.best_witness_idx, Some(0));
    assert_eq!(cert.certified_value(), Some(5));
}

#[test]
fn test_certificate_update_best_witness() {
    let mut cert = CutCertificate::new();
    let witness1 = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2]), 10);
    let witness2 = WitnessHandle::new(2, RoaringBitmap::from_iter([2, 3, 4]), 5);

    cert.set_best_witness(0, witness1);
    cert.set_best_witness(1, witness2.clone());

    // Best witness should be the one at index 1 with boundary 5
    let best = cert.best_witness().unwrap();
    assert_eq!(best.boundary_size(), 5);
    assert_eq!(cert.certified_value(), Some(5));
}

#[test]
fn test_certificate_add_response() {
    let mut cert = CutCertificate::new();
    let query = CertLocalKCutQuery::new(vec![1, 2, 3], 10, 5);
    let result = LocalKCutResultSummary::Found {
        cut_value: 5,
        witness_hash: 12345,
    };
    let response = LocalKCutResponse::new(query, result, 100, None);

    cert.add_response(response);

    assert_eq!(cert.num_responses(), 1);
}

#[test]
fn test_certificate_add_multiple_responses() {
    let mut cert = CutCertificate::new();

    for i in 0..5 {
        let query = CertLocalKCutQuery::new(vec![i], 10, 3);
        let result = LocalKCutResultSummary::Found {
            cut_value: i,
            witness_hash: i * 1000,
        };
        let response = LocalKCutResponse::new(query, result, i * 100, None);
        cert.add_response(response);
    }

    assert_eq!(cert.num_responses(), 5);
}

#[test]
fn test_certificate_verify_empty() {
    let cert = CutCertificate::new();
    let result = cert.verify();

    assert!(matches!(result, Err(CertificateError::NoWitness)));
}

#[test]
fn test_certificate_verify_valid() {
    let mut cert = CutCertificate::new();
    let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);
    cert.set_best_witness(0, witness);

    assert!(cert.verify().is_ok());
}

#[test]
fn test_certificate_verify_invalid_index() {
    let mut cert = CutCertificate::new();
    // Add a witness so the certificate is not empty
    let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2]), 5);
    cert.set_best_witness(0, witness);
    // Now set an invalid index
    cert.best_witness_idx = Some(10);

    let result = cert.verify();
    assert!(matches!(
        result,
        Err(CertificateError::InvalidWitnessIndex { .. })
    ));
}

#[test]
fn test_certificate_json_export() {
    let mut cert = CutCertificate::new();
    let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);
    cert.set_best_witness(0, witness);

    let json = cert.to_json().unwrap();

    assert!(json.contains("witness_summaries"));
    assert!(json.contains("localkcut_responses"));
    assert!(json.contains("version"));
}

#[test]
fn test_certificate_json_roundtrip() {
    let mut cert = CutCertificate::new();
    let witness1 = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2]), 5);
    let witness2 = WitnessHandle::new(2, RoaringBitmap::from_iter([2, 3, 4]), 3);

    cert.set_best_witness(0, witness1);
    cert.set_best_witness(1, witness2);

    let query = CertLocalKCutQuery::new(vec![1], 5, 2);
    let result = LocalKCutResultSummary::Found {
        cut_value: 3,
        witness_hash: 999,
    };
    let response = LocalKCutResponse::new(query, result, 100, None);
    cert.add_response(response);

    let json = cert.to_json().unwrap();
    let cert2 = CutCertificate::from_json(&json).unwrap();

    // Witnesses are not serialized, only summaries
    assert_eq!(cert2.witness_summaries.len(), 2);
    assert_eq!(cert2.num_responses(), 1);
    // certified_value requires actual witnesses, not just summaries
    assert!(cert2.witness_summaries.iter().any(|w| w.boundary == 3));
}

#[test]
fn test_audit_logger_creation() {
    let logger = AuditLogger::new(100);
    assert_eq!(logger.capacity(), 100);
    assert_eq!(logger.len(), 0);
    assert!(logger.is_empty());
}

#[test]
fn test_audit_logger_log_witness() {
    let logger = AuditLogger::new(100);
    let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);

    logger.log_witness_created(&witness);

    assert_eq!(logger.len(), 1);

    let entries = logger.by_type(AuditEntryType::WitnessCreated);
    assert_eq!(entries.len(), 1);
}

#[test]
fn test_audit_logger_log_query() {
    let logger = AuditLogger::new(100);

    logger.log_query(10, 5, vec![1, 2, 3]);

    let entries = logger.by_type(AuditEntryType::LocalKCutQuery);
    assert_eq!(entries.len(), 1);

    if let AuditData::Query {
        budget,
        radius,
        seeds,
    } = &entries[0].data
    {
        assert_eq!(*budget, 10);
        assert_eq!(*radius, 5);
        assert_eq!(seeds.len(), 3);
    } else {
        panic!("Expected Query data");
    }
}

#[test]
fn test_audit_logger_log_response() {
    let logger = AuditLogger::new(100);
    let query = CertLocalKCutQuery::new(vec![1], 5, 2);
    let result = LocalKCutResultSummary::Found {
        cut_value: 3,
        witness_hash: 999,
    };
    let response = LocalKCutResponse::new(query, result, 100, None);

    logger.log_response(&response);

    let entries = logger.by_type(AuditEntryType::LocalKCutResponse);
    assert_eq!(entries.len(), 1);
}

#[test]
fn test_audit_logger_log_mincut_change() {
    let logger = AuditLogger::new(100);
    let trigger = UpdateTrigger::new(UpdateType::Insert, 123, (1, 2), 1000);

    logger.log_mincut_changed(10, 8, trigger);

    let entries = logger.by_type(AuditEntryType::MinCutChanged);
    assert_eq!(entries.len(), 1);

    if let AuditData::MinCut {
        old_value,
        new_value,
        ..
    } = &entries[0].data
    {
        assert_eq!(*old_value, 10);
        assert_eq!(*new_value, 8);
    } else {
        panic!("Expected MinCut data");
    }
}

#[test]
fn test_audit_logger_max_capacity() {
    let logger = AuditLogger::new(3);

    for i in 0..10 {
        let witness =
            WitnessHandle::new(i, RoaringBitmap::from_iter([i as u32, (i + 1) as u32]), i);
        logger.log_witness_created(&witness);
    }

    // Should only keep last 3 entries
    assert_eq!(logger.len(), 3);
}

#[test]
fn test_audit_logger_recent() {
    let logger = AuditLogger::new(100);

    for i in 0..10 {
        let witness = WitnessHandle::new(i, RoaringBitmap::from_iter([i as u32]), i);
        logger.log_witness_created(&witness);
    }

    let recent = logger.recent(5);
    assert_eq!(recent.len(), 5);

    // Should be entries 5-9
    assert!(recent[0].id >= 5);
}

#[test]
fn test_audit_logger_clear() {
    let logger = AuditLogger::new(100);

    let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2]), 3);
    logger.log_witness_created(&witness);

    assert_eq!(logger.len(), 1);

    logger.clear();

    assert_eq!(logger.len(), 0);
    assert!(logger.is_empty());
}

#[test]
fn test_audit_logger_export() {
    let logger = AuditLogger::new(100);

    for i in 0..5 {
        let witness = WitnessHandle::new(i, RoaringBitmap::from_iter([i as u32]), i);
        logger.log_witness_created(&witness);
    }

    let exported = logger.export();
    assert_eq!(exported.len(), 5);
}

#[test]
fn test_audit_logger_json_export() {
    let logger = AuditLogger::new(100);

    let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);
    logger.log_witness_created(&witness);

    let json = logger.to_json().unwrap();
    assert!(json.contains("WitnessCreated"));
}

#[test]
fn test_certificate_with_audit_trail() {
    let logger = AuditLogger::new(1000);
    let mut cert = CutCertificate::new();

    // Log witness creation
    let witness1 = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2]), 10);
    logger.log_witness_created(&witness1);
    cert.set_best_witness(0, witness1);

    // Log witness update
    let witness2 = WitnessHandle::new(2, RoaringBitmap::from_iter([2, 3, 4]), 5);
    logger.log_witness_updated(&witness2);
    cert.set_best_witness(1, witness2);

    // Log query and response
    let query = CertLocalKCutQuery::new(vec![1, 2], 10, 5);
    logger.log_query(10, 5, vec![1, 2]);

    let result = LocalKCutResultSummary::Found {
        cut_value: 5,
        witness_hash: 12345,
    };
    let response = LocalKCutResponse::new(query, result, 100, None);
    logger.log_response(&response);
    cert.add_response(response);

    // Log certificate creation
    logger.log_certificate_created(
        cert.num_witnesses(),
        cert.num_responses(),
        cert.certified_value(),
    );

    // Verify audit trail
    assert_eq!(logger.len(), 5);

    let created = logger.by_type(AuditEntryType::WitnessCreated);
    assert_eq!(created.len(), 1);

    let updated = logger.by_type(AuditEntryType::WitnessUpdated);
    assert_eq!(updated.len(), 1);

    let queries = logger.by_type(AuditEntryType::LocalKCutQuery);
    assert_eq!(queries.len(), 1);

    let responses = logger.by_type(AuditEntryType::LocalKCutResponse);
    assert_eq!(responses.len(), 1);

    let certs = logger.by_type(AuditEntryType::CertificateCreated);
    assert_eq!(certs.len(), 1);

    // Verify certificate
    assert!(cert.verify().is_ok());
    assert_eq!(cert.certified_value(), Some(5));
}

#[test]
fn test_update_trigger_creation() {
    let trigger = UpdateTrigger::new(UpdateType::Insert, 123, (1, 2), 1000);

    assert_eq!(trigger.update_type, UpdateType::Insert);
    assert_eq!(trigger.edge_id, 123);
    assert_eq!(trigger.endpoints, (1, 2));
    assert_eq!(trigger.time, 1000);
}

#[test]
fn test_update_type_equality() {
    assert_eq!(UpdateType::Insert, UpdateType::Insert);
    assert_eq!(UpdateType::Delete, UpdateType::Delete);
    assert_ne!(UpdateType::Insert, UpdateType::Delete);
}

#[test]
fn test_local_kcut_query_creation() {
    let query = CertLocalKCutQuery::new(vec![1, 2, 3], 10, 5);

    assert_eq!(query.seed_vertices.len(), 3);
    assert_eq!(query.budget_k, 10);
    assert_eq!(query.radius, 5);
}

#[test]
fn test_local_kcut_result_summary() {
    let result_found = LocalKCutResultSummary::Found {
        cut_value: 5,
        witness_hash: 12345,
    };

    if let LocalKCutResultSummary::Found { cut_value, .. } = result_found {
        assert_eq!(cut_value, 5);
    } else {
        panic!("Expected Found variant");
    }

    let result_none = LocalKCutResultSummary::NoneInLocality;
    assert!(matches!(
        result_none,
        LocalKCutResultSummary::NoneInLocality
    ));
}

#[test]
fn test_certificate_error_display() {
    let err = CertificateError::NoWitness;
    assert!(err.to_string().contains("No witness"));

    let err = CertificateError::InvalidWitnessIndex { index: 5, max: 3 };
    assert!(err.to_string().contains("Invalid witness index"));

    let err = CertificateError::InconsistentBoundary {
        expected: 10,
        actual: 5,
    };
    assert!(err.to_string().contains("Inconsistent boundary"));
}

#[test]
fn test_full_certificate_workflow() {
    // Create certificate
    let mut cert = CutCertificate::new();

    // Add witnesses from different sources
    let witness1 = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 8);
    let witness2 = WitnessHandle::new(2, RoaringBitmap::from_iter([2, 4, 5]), 5);
    let witness3 = WitnessHandle::new(3, RoaringBitmap::from_iter([3, 6, 7, 8, 9]), 12);

    cert.set_best_witness(0, witness1);
    cert.set_best_witness(1, witness2);
    cert.set_best_witness(2, witness3);

    // Add LocalKCut responses
    for i in 1..=3 {
        let query = CertLocalKCutQuery::new(vec![i], i * 5, 3);
        let result = if i == 2 {
            LocalKCutResultSummary::Found {
                cut_value: 5,
                witness_hash: i * 1000,
            }
        } else {
            LocalKCutResultSummary::NoneInLocality
        };
        let trigger = UpdateTrigger::new(UpdateType::Insert, i, (i, i + 1), i * 100);
        let response = LocalKCutResponse::new(query, result, i * 100, Some(trigger));
        cert.add_response(response);
    }

    // Verify certificate
    assert!(cert.verify().is_ok());
    assert_eq!(cert.num_witnesses(), 3);
    assert_eq!(cert.num_responses(), 3);
    assert_eq!(cert.certified_value(), Some(12)); // Last set witness

    // Export to JSON
    let json = cert.to_json().unwrap();

    // Import from JSON
    let cert2 = CutCertificate::from_json(&json).unwrap();

    // Verify imported certificate
    assert!(cert2.verify().is_ok());
    // Witnesses are not serialized, only summaries
    assert_eq!(cert2.witness_summaries.len(), cert.witness_summaries.len());
    assert_eq!(cert2.num_responses(), cert.num_responses());
}
