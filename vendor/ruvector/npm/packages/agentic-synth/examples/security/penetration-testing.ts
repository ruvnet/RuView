/**
 * Penetration Testing Data Examples
 *
 * ⚠️ ETHICAL USE ONLY ⚠️
 * These examples are for:
 * - Authorized penetration testing engagements
 * - Red team exercises in controlled environments
 * - Security tool development and validation
 * - Penetration testing training and certification
 *
 * ALWAYS obtain written authorization before testing.
 */

import { AgenticSynth } from 'agentic-synth';

/**
 * Network Scanning Results
 * For testing vulnerability scanners and network mapping tools
 */
export async function generateNetworkScanResults() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const scanPrompt = `
Generate network scanning results data for penetration testing.
Include host discovery, port scans, service detection results.
Each scan result should have: target, ports, services, vulnerabilities, recommendations.
Generate 10 diverse network scan results.
  `;

  const results = await synth.generate({
    prompt: scanPrompt,
    schema: {
      type: 'object',
      properties: {
        scan_results: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              scan_id: { type: 'string' },
              scan_date: { type: 'string' },
              target: {
                type: 'object',
                properties: {
                  ip_address: { type: 'string' },
                  hostname: { type: 'string' },
                  mac_address: { type: 'string' },
                  operating_system: { type: 'string' },
                  os_confidence: { type: 'number' }
                }
              },
              scan_type: {
                type: 'string',
                enum: ['tcp_connect', 'syn_scan', 'udp_scan', 'comprehensive', 'stealth']
              },
              open_ports: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    port: { type: 'number' },
                    protocol: { type: 'string' },
                    state: { type: 'string' },
                    service: { type: 'string' },
                    version: { type: 'string' },
                    banner: { type: 'string' }
                  }
                }
              },
              filtered_ports: { type: 'array', items: { type: 'number' } },
              services_detected: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    service_name: { type: 'string' },
                    version: { type: 'string' },
                    cpe: { type: 'string' },
                    product: { type: 'string' },
                    extra_info: { type: 'string' }
                  }
                }
              },
              vulnerabilities_found: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    cve_id: { type: 'string' },
                    severity: { type: 'string' },
                    cvss_score: { type: 'number' },
                    description: { type: 'string' },
                    affected_service: { type: 'string' }
                  }
                }
              },
              firewall_detected: { type: 'boolean' },
              ids_ips_detected: { type: 'boolean' },
              recommendations: { type: 'array', items: { type: 'string' } }
            },
            required: ['id', 'target', 'scan_type', 'open_ports']
          }
        }
      },
      required: ['scan_results']
    }
  });

  return results;
}

/**
 * Port Enumeration Data
 * For testing port scanning tools and service identification
 */
export async function generatePortEnumerationData() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const portPrompt = `
Generate port enumeration data for penetration testing tools.
Include common services, uncommon ports, misconfigurations.
Each enumeration should have: port_info, service_details, security_findings.
Generate 12 port enumeration scenarios.
  `;

  const data = await synth.generate({
    prompt: portPrompt,
    schema: {
      type: 'object',
      properties: {
        enumerations: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              target_ip: { type: 'string' },
              port: { type: 'number' },
              protocol: { type: 'string', enum: ['tcp', 'udp'] },
              state: { type: 'string', enum: ['open', 'closed', 'filtered'] },
              service_detection: {
                type: 'object',
                properties: {
                  service_name: { type: 'string' },
                  product: { type: 'string' },
                  version: { type: 'string' },
                  os_type: { type: 'string' },
                  device_type: { type: 'string' },
                  banner_grab: { type: 'string' }
                }
              },
              detailed_analysis: {
                type: 'object',
                properties: {
                  ssl_tls_info: {
                    type: 'object',
                    properties: {
                      enabled: { type: 'boolean' },
                      version: { type: 'string' },
                      cipher_suites: { type: 'array', items: { type: 'string' } },
                      certificate_info: { type: 'string' },
                      vulnerabilities: { type: 'array', items: { type: 'string' } }
                    }
                  },
                  authentication: {
                    type: 'object',
                    properties: {
                      required: { type: 'boolean' },
                      methods: { type: 'array', items: { type: 'string' } },
                      default_credentials_tested: { type: 'boolean' },
                      weak_auth_detected: { type: 'boolean' }
                    }
                  }
                }
              },
              security_findings: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    finding_type: { type: 'string' },
                    severity: { type: 'string' },
                    description: { type: 'string' },
                    exploitation_difficulty: { type: 'string' }
                  }
                }
              },
              exploitation_potential: { type: 'string' },
              recommended_tests: { type: 'array', items: { type: 'string' } }
            },
            required: ['id', 'target_ip', 'port', 'service_detection']
          }
        }
      },
      required: ['enumerations']
    }
  });

  return data;
}

/**
 * Service Fingerprinting Data
 * For testing service identification and version detection
 */
export async function generateServiceFingerprints() {
  const synth = new AgenticSynth({
    temperature: 0.8,
    maxRetries: 3
  });

  const fingerprintPrompt = `
Generate service fingerprinting data for penetration testing.
Include web servers, databases, mail servers, authentication services.
Each fingerprint should have: service_type, version_info, vulnerabilities, attack_vectors.
Generate 10 service fingerprint scenarios.
  `;

  const fingerprints = await synth.generate({
    prompt: fingerprintPrompt,
    schema: {
      type: 'object',
      properties: {
        fingerprints: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              service_category: {
                type: 'string',
                enum: [
                  'web_server',
                  'database',
                  'mail_server',
                  'file_server',
                  'authentication_service',
                  'application_server',
                  'message_queue',
                  'cache_server'
                ]
              },
              service_info: {
                type: 'object',
                properties: {
                  name: { type: 'string' },
                  vendor: { type: 'string' },
                  version: { type: 'string' },
                  build_number: { type: 'string' },
                  release_date: { type: 'string' }
                }
              },
              detection_methods: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    method: { type: 'string' },
                    confidence: { type: 'number' },
                    evidence: { type: 'string' }
                  }
                }
              },
              known_vulnerabilities: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    cve_id: { type: 'string' },
                    cvss_score: { type: 'number' },
                    exploit_available: { type: 'boolean' },
                    metasploit_module: { type: 'string' },
                    description: { type: 'string' }
                  }
                }
              },
              configuration_issues: { type: 'array', items: { type: 'string' } },
              attack_vectors: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    vector_name: { type: 'string' },
                    difficulty: { type: 'string' },
                    impact: { type: 'string' },
                    prerequisites: { type: 'array', items: { type: 'string' } }
                  }
                }
              },
              exploitation_notes: { type: 'string' },
              recommended_patches: { type: 'array', items: { type: 'string' } }
            },
            required: ['id', 'service_category', 'service_info', 'attack_vectors']
          }
        }
      },
      required: ['fingerprints']
    }
  });

  return fingerprints;
}

/**
 * Exploitation Attempt Logs
 * For testing exploit detection and prevention systems
 */
export async function generateExploitationLogs() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const exploitPrompt = `
Generate exploitation attempt logs for security testing.
Include buffer overflows, code injection, privilege escalation attempts.
Each log should have: exploit_type, payload, success_status, detection_status.
Generate 12 exploitation attempt scenarios.
  `;

  const logs = await synth.generate({
    prompt: exploitPrompt,
    schema: {
      type: 'object',
      properties: {
        exploitation_attempts: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              timestamp: { type: 'string' },
              exploit_type: {
                type: 'string',
                enum: [
                  'buffer_overflow',
                  'sql_injection',
                  'command_injection',
                  'remote_code_execution',
                  'privilege_escalation',
                  'authentication_bypass',
                  'directory_traversal',
                  'deserialization',
                  'xxe_injection'
                ]
              },
              target: {
                type: 'object',
                properties: {
                  ip: { type: 'string' },
                  port: { type: 'number' },
                  service: { type: 'string' },
                  endpoint: { type: 'string' }
                }
              },
              exploit_details: {
                type: 'object',
                properties: {
                  cve_id: { type: 'string' },
                  exploit_name: { type: 'string' },
                  exploit_framework: { type: 'string' },
                  payload_type: { type: 'string' },
                  shellcode_used: { type: 'boolean' }
                }
              },
              payload_info: {
                type: 'object',
                properties: {
                  payload_size: { type: 'number' },
                  encoding: { type: 'string' },
                  obfuscation: { type: 'boolean' },
                  delivery_method: { type: 'string' }
                }
              },
              execution_result: {
                type: 'object',
                properties: {
                  success: { type: 'boolean' },
                  error_message: { type: 'string' },
                  shell_obtained: { type: 'boolean' },
                  privileges_gained: { type: 'string' },
                  access_level: { type: 'string' }
                }
              },
              detection_status: {
                type: 'object',
                properties: {
                  detected: { type: 'boolean' },
                  detection_method: { type: 'string' },
                  blocked: { type: 'boolean' },
                  alert_generated: { type: 'boolean' }
                }
              },
              post_exploitation: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    action: { type: 'string' },
                    timestamp: { type: 'string' },
                    success: { type: 'boolean' }
                  }
                }
              },
              remediation: { type: 'string' }
            },
            required: ['id', 'exploit_type', 'target', 'execution_result', 'detection_status']
          }
        }
      },
      required: ['exploitation_attempts']
    }
  });

  return logs;
}

/**
 * Post-Exploitation Activity Simulation
 * For testing lateral movement and persistence detection
 */
export async function generatePostExploitationActivity() {
  const synth = new AgenticSynth({
    temperature: 0.8,
    maxRetries: 3
  });

  const postExploitPrompt = `
Generate post-exploitation activity data for security testing.
Include lateral movement, privilege escalation, persistence mechanisms, data exfiltration.
Each activity should have: technique, commands, indicators, detection_opportunities.
Generate 10 post-exploitation scenarios following MITRE ATT&CK.
  `;

  const activities = await synth.generate({
    prompt: postExploitPrompt,
    schema: {
      type: 'object',
      properties: {
        activities: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              scenario_name: { type: 'string' },
              initial_access: {
                type: 'object',
                properties: {
                  method: { type: 'string' },
                  compromised_host: { type: 'string' },
                  initial_privileges: { type: 'string' },
                  timestamp: { type: 'string' }
                }
              },
              activity_chain: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    sequence: { type: 'number' },
                    mitre_technique: { type: 'string' },
                    tactic: { type: 'string' },
                    technique_name: { type: 'string' },
                    description: { type: 'string' },
                    commands_executed: { type: 'array', items: { type: 'string' } },
                    tools_used: { type: 'array', items: { type: 'string' } },
                    artifacts_created: { type: 'array', items: { type: 'string' } },
                    network_connections: {
                      type: 'array',
                      items: {
                        type: 'object',
                        properties: {
                          source: { type: 'string' },
                          destination: { type: 'string' },
                          port: { type: 'number' },
                          protocol: { type: 'string' }
                        }
                      }
                    }
                  }
                }
              },
              persistence_mechanisms: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    method: { type: 'string' },
                    location: { type: 'string' },
                    trigger: { type: 'string' },
                    stealth_level: { type: 'string' }
                  }
                }
              },
              lateral_movement: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    from_host: { type: 'string' },
                    to_host: { type: 'string' },
                    method: { type: 'string' },
                    credentials_used: { type: 'string' },
                    success: { type: 'boolean' }
                  }
                }
              },
              data_exfiltration: {
                type: 'object',
                properties: {
                  occurred: { type: 'boolean' },
                  data_types: { type: 'array', items: { type: 'string' } },
                  volume_mb: { type: 'number' },
                  exfil_method: { type: 'string' },
                  c2_server: { type: 'string' }
                }
              },
              detection_opportunities: { type: 'array', items: { type: 'string' } },
              indicators_of_compromise: { type: 'array', items: { type: 'string' } },
              defensive_recommendations: { type: 'array', items: { type: 'string' } }
            },
            required: ['id', 'scenario_name', 'initial_access', 'activity_chain']
          }
        }
      },
      required: ['activities']
    }
  });

  return activities;
}

/**
 * Penetration Testing Report Data
 * For testing reporting systems and findings management
 */
export async function generatePentestReportData() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const reportPrompt = `
Generate penetration testing report data with findings and recommendations.
Include executive summary metrics, technical findings, risk ratings, remediation plans.
Each report should have: engagement_info, findings, risk_analysis, recommendations.
Generate 5 comprehensive pentest report datasets.
  `;

  const reports = await synth.generate({
    prompt: reportPrompt,
    schema: {
      type: 'object',
      properties: {
        reports: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              engagement_info: {
                type: 'object',
                properties: {
                  client_name: { type: 'string' },
                  engagement_type: { type: 'string' },
                  test_date_range: { type: 'string' },
                  scope: { type: 'array', items: { type: 'string' } },
                  testing_methodology: { type: 'string' },
                  rules_of_engagement: { type: 'string' }
                }
              },
              executive_summary: {
                type: 'object',
                properties: {
                  total_findings: { type: 'number' },
                  critical_findings: { type: 'number' },
                  high_findings: { type: 'number' },
                  medium_findings: { type: 'number' },
                  low_findings: { type: 'number' },
                  overall_risk_rating: { type: 'string' },
                  key_observations: { type: 'array', items: { type: 'string' } }
                }
              },
              findings: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    finding_id: { type: 'string' },
                    title: { type: 'string' },
                    severity: { type: 'string' },
                    cvss_score: { type: 'number' },
                    affected_systems: { type: 'array', items: { type: 'string' } },
                    description: { type: 'string' },
                    impact: { type: 'string' },
                    likelihood: { type: 'string' },
                    evidence: { type: 'array', items: { type: 'string' } },
                    remediation: { type: 'string' },
                    remediation_priority: { type: 'string' },
                    references: { type: 'array', items: { type: 'string' } }
                  }
                }
              },
              recommendations: { type: 'array', items: { type: 'string' } },
              conclusion: { type: 'string' }
            },
            required: ['id', 'engagement_info', 'executive_summary', 'findings']
          }
        }
      },
      required: ['reports']
    }
  });

  return reports;
}

/**
 * Example Usage
 */
export async function runPenetrationTests() {
  console.log('⚠️  Running Authorized Penetration Testing Data Generation ⚠️\n');

  try {
    // Generate network scan results
    console.log('Generating network scan results...');
    const scanResults = await generateNetworkScanResults();
    console.log(`Generated ${scanResults.scan_results?.length || 0} scan results\n`);

    // Generate port enumeration data
    console.log('Generating port enumeration data...');
    const portData = await generatePortEnumerationData();
    console.log(`Generated ${portData.enumerations?.length || 0} port enumerations\n`);

    // Generate service fingerprints
    console.log('Generating service fingerprints...');
    const fingerprints = await generateServiceFingerprints();
    console.log(`Generated ${fingerprints.fingerprints?.length || 0} service fingerprints\n`);

    // Generate exploitation logs
    console.log('Generating exploitation attempt logs...');
    const exploitLogs = await generateExploitationLogs();
    console.log(`Generated ${exploitLogs.exploitation_attempts?.length || 0} exploitation logs\n`);

    // Generate post-exploitation activities
    console.log('Generating post-exploitation activities...');
    const postExploit = await generatePostExploitationActivity();
    console.log(`Generated ${postExploit.activities?.length || 0} post-exploitation scenarios\n`);

    // Generate pentest reports
    console.log('Generating penetration testing reports...');
    const reports = await generatePentestReportData();
    console.log(`Generated ${reports.reports?.length || 0} pentest reports\n`);

    return {
      scanResults,
      portData,
      fingerprints,
      exploitLogs,
      postExploit,
      reports
    };
  } catch (error) {
    console.error('Error generating penetration testing data:', error);
    throw error;
  }
}

// Export all generators
export default {
  generateNetworkScanResults,
  generatePortEnumerationData,
  generateServiceFingerprints,
  generateExploitationLogs,
  generatePostExploitationActivity,
  generatePentestReportData,
  runPenetrationTests
};
