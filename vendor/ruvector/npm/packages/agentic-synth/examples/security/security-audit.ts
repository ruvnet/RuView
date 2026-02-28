/**
 * Security Audit Data Examples
 *
 * ⚠️ ETHICAL USE ONLY ⚠️
 * These examples are for:
 * - Security Information and Event Management (SIEM) testing
 * - Compliance auditing and reporting
 * - Security monitoring system validation
 * - Incident investigation training
 *
 * For authorized security operations only.
 */

import { AgenticSynth } from 'agentic-synth';

/**
 * User Access Pattern Analysis
 * For detecting suspicious access patterns and privilege escalation
 */
export async function generateUserAccessPatterns() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const accessPrompt = `
Generate user access pattern data for security audit analysis.
Include normal patterns, anomalous patterns, suspicious activities.
Each pattern should have: user_id, access_events, risk_score, anomaly_indicators.
Generate 15 diverse user access patterns including both normal and suspicious.
  `;

  const patterns = await synth.generate({
    prompt: accessPrompt,
    schema: {
      type: 'object',
      properties: {
        access_patterns: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              user_id: { type: 'string' },
              user_role: { type: 'string' },
              time_period: {
                type: 'object',
                properties: {
                  start_date: { type: 'string' },
                  end_date: { type: 'string' },
                  total_days: { type: 'number' }
                }
              },
              access_events: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    timestamp: { type: 'string' },
                    resource: { type: 'string' },
                    action: { type: 'string' },
                    result: { type: 'string', enum: ['success', 'failure', 'denied'] },
                    source_ip: { type: 'string' },
                    user_agent: { type: 'string' },
                    geolocation: { type: 'string' },
                    sensitivity_level: { type: 'string' }
                  }
                }
              },
              behavioral_metrics: {
                type: 'object',
                properties: {
                  total_accesses: { type: 'number' },
                  unique_resources: { type: 'number' },
                  failed_attempts: { type: 'number' },
                  off_hours_access: { type: 'number' },
                  unusual_locations: { type: 'number' },
                  data_download_volume_mb: { type: 'number' }
                }
              },
              anomaly_indicators: { type: 'array', items: { type: 'string' } },
              risk_score: { type: 'number', minimum: 0, maximum: 100 },
              classification: {
                type: 'string',
                enum: ['normal', 'suspicious', 'high_risk', 'critical']
              },
              recommended_actions: { type: 'array', items: { type: 'string' } }
            },
            required: ['id', 'user_id', 'access_events', 'risk_score', 'classification']
          }
        }
      },
      required: ['access_patterns']
    }
  });

  return patterns;
}

/**
 * Permission Change Audit Trail
 * For tracking privilege escalations and access control modifications
 */
export async function generatePermissionChangeAudits() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const permissionPrompt = `
Generate permission change audit data for security compliance.
Include role modifications, privilege escalations, group membership changes.
Each audit entry should have: change_type, before_state, after_state, approvals, compliance_flags.
Generate 12 permission change audit scenarios.
  `;

  const audits = await synth.generate({
    prompt: permissionPrompt,
    schema: {
      type: 'object',
      properties: {
        permission_changes: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              change_type: {
                type: 'string',
                enum: [
                  'role_assignment',
                  'role_removal',
                  'permission_grant',
                  'permission_revoke',
                  'group_membership',
                  'privilege_escalation',
                  'access_scope_change'
                ]
              },
              timestamp: { type: 'string' },
              modified_by: { type: 'string' },
              modified_for: { type: 'string' },
              before_state: {
                type: 'object',
                properties: {
                  roles: { type: 'array', items: { type: 'string' } },
                  permissions: { type: 'array', items: { type: 'string' } },
                  groups: { type: 'array', items: { type: 'string' } },
                  access_level: { type: 'string' }
                }
              },
              after_state: {
                type: 'object',
                properties: {
                  roles: { type: 'array', items: { type: 'string' } },
                  permissions: { type: 'array', items: { type: 'string' } },
                  groups: { type: 'array', items: { type: 'string' } },
                  access_level: { type: 'string' }
                }
              },
              justification: { type: 'string' },
              approval_workflow: {
                type: 'object',
                properties: {
                  required: { type: 'boolean' },
                  approved_by: { type: 'array', items: { type: 'string' } },
                  approval_date: { type: 'string' },
                  ticket_reference: { type: 'string' }
                }
              },
              compliance_flags: { type: 'array', items: { type: 'string' } },
              risk_assessment: {
                type: 'string',
                enum: ['low', 'medium', 'high', 'critical']
              },
              requires_review: { type: 'boolean' },
              audit_notes: { type: 'string' }
            },
            required: ['id', 'change_type', 'modified_by', 'before_state', 'after_state']
          }
        }
      },
      required: ['permission_changes']
    }
  });

  return audits;
}

/**
 * Configuration Change Monitoring
 * For tracking security-sensitive configuration modifications
 */
export async function generateConfigurationChangeAudits() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const configPrompt = `
Generate configuration change audit data for security monitoring.
Include firewall rules, security policies, encryption settings, authentication configs.
Each change should have: config_type, change_details, security_impact, compliance_status.
Generate 10 configuration change audit entries.
  `;

  const audits = await synth.generate({
    prompt: configPrompt,
    schema: {
      type: 'object',
      properties: {
        config_changes: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              config_type: {
                type: 'string',
                enum: [
                  'firewall_rule',
                  'security_policy',
                  'encryption_setting',
                  'authentication_method',
                  'network_configuration',
                  'access_control_list',
                  'logging_configuration',
                  'certificate_management'
                ]
              },
              timestamp: { type: 'string' },
              system: { type: 'string' },
              component: { type: 'string' },
              changed_by: { type: 'string' },
              change_method: { type: 'string' },
              change_details: {
                type: 'object',
                properties: {
                  parameter: { type: 'string' },
                  old_value: { type: 'string' },
                  new_value: { type: 'string' },
                  config_file: { type: 'string' }
                }
              },
              security_impact: {
                type: 'object',
                properties: {
                  impact_level: {
                    type: 'string',
                    enum: ['none', 'low', 'medium', 'high', 'critical']
                  },
                  affected_systems: { type: 'array', items: { type: 'string' } },
                  attack_surface_change: { type: 'string' },
                  mitigation_effectiveness: { type: 'string' }
                }
              },
              compliance_status: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    framework: { type: 'string' },
                    requirement: { type: 'string' },
                    status: { type: 'string', enum: ['compliant', 'non_compliant', 'review_required'] }
                  }
                }
              },
              validation_status: { type: 'string' },
              rollback_available: { type: 'boolean' },
              audit_trail: { type: 'array', items: { type: 'string' } }
            },
            required: ['id', 'config_type', 'changed_by', 'change_details', 'security_impact']
          }
        }
      },
      required: ['config_changes']
    }
  });

  return audits;
}

/**
 * Compliance Violation Scenarios
 * For testing compliance monitoring and alerting systems
 */
export async function generateComplianceViolations() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const compliancePrompt = `
Generate compliance violation scenarios for security audit testing.
Include GDPR, HIPAA, PCI-DSS, SOX violations.
Each violation should have: framework, requirement, violation_details, severity, remediation.
Generate 10 compliance violation scenarios.
  `;

  const violations = await synth.generate({
    prompt: compliancePrompt,
    schema: {
      type: 'object',
      properties: {
        violations: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              compliance_framework: {
                type: 'string',
                enum: ['GDPR', 'HIPAA', 'PCI_DSS', 'SOX', 'ISO_27001', 'NIST', 'SOC2', 'CCPA']
              },
              requirement_id: { type: 'string' },
              requirement_description: { type: 'string' },
              violation_details: {
                type: 'object',
                properties: {
                  detected_date: { type: 'string' },
                  detection_method: { type: 'string' },
                  affected_systems: { type: 'array', items: { type: 'string' } },
                  violation_type: { type: 'string' },
                  description: { type: 'string' },
                  evidence: { type: 'array', items: { type: 'string' } }
                }
              },
              severity: {
                type: 'string',
                enum: ['low', 'medium', 'high', 'critical']
              },
              potential_penalties: { type: 'string' },
              affected_records: { type: 'number' },
              business_impact: { type: 'string' },
              remediation: {
                type: 'object',
                properties: {
                  required_actions: { type: 'array', items: { type: 'string' } },
                  timeline: { type: 'string' },
                  responsible_party: { type: 'string' },
                  status: { type: 'string' }
                }
              },
              notification_required: { type: 'boolean' },
              audit_findings: { type: 'array', items: { type: 'string' } }
            },
            required: ['id', 'compliance_framework', 'violation_details', 'severity']
          }
        }
      },
      required: ['violations']
    }
  });

  return violations;
}

/**
 * Security Event Correlation Data
 * For SIEM correlation rules and incident detection
 */
export async function generateSecurityEventCorrelations() {
  const synth = new AgenticSynth({
    temperature: 0.8,
    maxRetries: 3
  });

  const correlationPrompt = `
Generate security event correlation data for SIEM testing.
Include multi-stage attacks, lateral movement, data exfiltration chains.
Each correlation should have: event_chain, attack_pattern, indicators, detection_logic.
Generate 8 security event correlation scenarios.
  `;

  const correlations = await synth.generate({
    prompt: correlationPrompt,
    schema: {
      type: 'object',
      properties: {
        correlations: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              attack_pattern: { type: 'string' },
              mitre_tactics: { type: 'array', items: { type: 'string' } },
              event_chain: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    sequence: { type: 'number' },
                    timestamp: { type: 'string' },
                    event_type: { type: 'string' },
                    source: { type: 'string' },
                    destination: { type: 'string' },
                    details: { type: 'string' },
                    severity: { type: 'string' }
                  }
                }
              },
              correlation_indicators: { type: 'array', items: { type: 'string' } },
              time_window: { type: 'string' },
              confidence_score: { type: 'number', minimum: 0, maximum: 100 },
              detection_logic: {
                type: 'object',
                properties: {
                  rule_description: { type: 'string' },
                  conditions: { type: 'array', items: { type: 'string' } },
                  threshold: { type: 'string' }
                }
              },
              false_positive_likelihood: { type: 'string' },
              recommended_response: { type: 'string' },
              investigation_steps: { type: 'array', items: { type: 'string' } }
            },
            required: ['id', 'attack_pattern', 'event_chain', 'detection_logic']
          }
        }
      },
      required: ['correlations']
    }
  });

  return correlations;
}

/**
 * Data Loss Prevention (DLP) Audit Data
 * For testing DLP policies and data classification
 */
export async function generateDLPAuditData() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const dlpPrompt = `
Generate Data Loss Prevention audit data for security testing.
Include sensitive data transfers, policy violations, data classification issues.
Each audit entry should have: data_type, transfer_method, policy_match, action_taken.
Generate 10 DLP audit scenarios.
  `;

  const audits = await synth.generate({
    prompt: dlpPrompt,
    schema: {
      type: 'object',
      properties: {
        dlp_events: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              timestamp: { type: 'string' },
              user: { type: 'string' },
              data_classification: {
                type: 'string',
                enum: ['public', 'internal', 'confidential', 'restricted', 'top_secret']
              },
              data_types_detected: { type: 'array', items: { type: 'string' } },
              transfer_method: {
                type: 'string',
                enum: ['email', 'usb', 'cloud_storage', 'web_upload', 'print', 'clipboard']
              },
              destination: { type: 'string' },
              file_info: {
                type: 'object',
                properties: {
                  filename: { type: 'string' },
                  size_mb: { type: 'number' },
                  type: { type: 'string' }
                }
              },
              policy_matched: { type: 'string' },
              violations: { type: 'array', items: { type: 'string' } },
              action_taken: {
                type: 'string',
                enum: ['allow', 'block', 'quarantine', 'encrypt', 'alert']
              },
              justification_provided: { type: 'boolean' },
              risk_score: { type: 'number' },
              requires_review: { type: 'boolean' },
              incident_created: { type: 'boolean' }
            },
            required: ['id', 'data_classification', 'transfer_method', 'action_taken']
          }
        }
      },
      required: ['dlp_events']
    }
  });

  return audits;
}

/**
 * Example Usage
 */
export async function runSecurityAudits() {
  console.log('⚠️  Running Security Audit Data Generation ⚠️\n');

  try {
    // Generate user access patterns
    console.log('Generating user access patterns...');
    const accessPatterns = await generateUserAccessPatterns();
    console.log(`Generated ${accessPatterns.access_patterns?.length || 0} access patterns\n`);

    // Generate permission changes
    console.log('Generating permission change audits...');
    const permissionChanges = await generatePermissionChangeAudits();
    console.log(`Generated ${permissionChanges.permission_changes?.length || 0} permission changes\n`);

    // Generate configuration changes
    console.log('Generating configuration change audits...');
    const configChanges = await generateConfigurationChangeAudits();
    console.log(`Generated ${configChanges.config_changes?.length || 0} config changes\n`);

    // Generate compliance violations
    console.log('Generating compliance violations...');
    const violations = await generateComplianceViolations();
    console.log(`Generated ${violations.violations?.length || 0} compliance violations\n`);

    // Generate event correlations
    console.log('Generating security event correlations...');
    const correlations = await generateSecurityEventCorrelations();
    console.log(`Generated ${correlations.correlations?.length || 0} event correlations\n`);

    // Generate DLP audit data
    console.log('Generating DLP audit data...');
    const dlpData = await generateDLPAuditData();
    console.log(`Generated ${dlpData.dlp_events?.length || 0} DLP events\n`);

    return {
      accessPatterns,
      permissionChanges,
      configChanges,
      violations,
      correlations,
      dlpData
    };
  } catch (error) {
    console.error('Error generating security audit data:', error);
    throw error;
  }
}

// Export all generators
export default {
  generateUserAccessPatterns,
  generatePermissionChangeAudits,
  generateConfigurationChangeAudits,
  generateComplianceViolations,
  generateSecurityEventCorrelations,
  generateDLPAuditData,
  runSecurityAudits
};
