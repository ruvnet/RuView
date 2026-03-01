/**
 * Threat Simulation Data Examples
 *
 * ⚠️ ETHICAL USE ONLY ⚠️
 * These simulations are for:
 * - Security operations center (SOC) training
 * - Incident response preparation
 * - Threat detection system validation
 * - Red team exercises in authorized environments
 *
 * NEVER use for actual attacks or unauthorized testing.
 */

import { AgenticSynth } from 'agentic-synth';

/**
 * Brute Force Attack Pattern Simulation
 * For testing account lockout and rate limiting mechanisms
 */
export async function generateBruteForcePatterns() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const bruteForcePrompt = `
Generate brute force attack pattern simulations for defensive security testing.
Include password spray, credential stuffing, dictionary attacks.
Each pattern should have: attack_type, target, timing, credentials_tested, detection_indicators.
Generate 10 realistic brute force attack patterns.
  `;

  const patterns = await synth.generate({
    prompt: bruteForcePrompt,
    schema: {
      type: 'object',
      properties: {
        patterns: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              attack_type: {
                type: 'string',
                enum: [
                  'password_spray',
                  'credential_stuffing',
                  'dictionary_attack',
                  'hybrid_attack',
                  'rainbow_table',
                  'reverse_brute_force'
                ]
              },
              target_service: { type: 'string' },
              target_endpoints: { type: 'array', items: { type: 'string' } },
              timing_pattern: {
                type: 'object',
                properties: {
                  attempts_per_minute: { type: 'number' },
                  delay_between_attempts: { type: 'number' },
                  total_duration_minutes: { type: 'number' },
                  distributed_sources: { type: 'boolean' }
                }
              },
              credentials_tested: { type: 'number' },
              usernames_targeted: { type: 'number' },
              source_ips: { type: 'array', items: { type: 'string' } },
              user_agents: { type: 'array', items: { type: 'string' } },
              detection_indicators: {
                type: 'array',
                items: { type: 'string' }
              },
              expected_defenses: {
                type: 'array',
                items: { type: 'string' }
              },
              severity: { type: 'string' }
            },
            required: ['id', 'attack_type', 'target_service', 'timing_pattern']
          }
        }
      },
      required: ['patterns']
    }
  });

  return patterns;
}

/**
 * DDoS Traffic Simulation Data
 * For testing DDoS mitigation and traffic filtering
 */
export async function generateDDoSSimulation() {
  const synth = new AgenticSynth({
    temperature: 0.8,
    maxRetries: 3
  });

  const ddosPrompt = `
Generate DDoS attack simulation data for defensive testing.
Include volumetric, protocol, and application layer attacks.
Each simulation should have: attack_vector, traffic_pattern, volume, mitigation_strategy.
Generate 8 different DDoS attack simulations.
  `;

  const simulations = await synth.generate({
    prompt: ddosPrompt,
    schema: {
      type: 'object',
      properties: {
        simulations: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              attack_vector: {
                type: 'string',
                enum: [
                  'syn_flood',
                  'udp_flood',
                  'http_flood',
                  'slowloris',
                  'dns_amplification',
                  'ntp_amplification',
                  'ssdp_amplification',
                  'memcached_amplification'
                ]
              },
              layer: {
                type: 'string',
                enum: ['layer3', 'layer4', 'layer7']
              },
              traffic_pattern: {
                type: 'object',
                properties: {
                  packets_per_second: { type: 'number' },
                  requests_per_second: { type: 'number' },
                  bandwidth_mbps: { type: 'number' },
                  source_ips: { type: 'number' },
                  botnet_size: { type: 'number' }
                }
              },
              target_resources: { type: 'array', items: { type: 'string' } },
              duration_minutes: { type: 'number' },
              amplification_factor: { type: 'number' },
              detection_signatures: { type: 'array', items: { type: 'string' } },
              mitigation_strategies: { type: 'array', items: { type: 'string' } },
              impact_severity: { type: 'string' }
            },
            required: ['id', 'attack_vector', 'layer', 'traffic_pattern']
          }
        }
      },
      required: ['simulations']
    }
  });

  return simulations;
}

/**
 * Malware Behavior Pattern Simulation
 * For testing endpoint detection and response (EDR) systems
 */
export async function generateMalwareBehaviors() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const malwarePrompt = `
Generate malware behavior patterns for EDR/XDR testing.
Include ransomware, trojans, rootkits, and APT behaviors.
Each behavior should have: malware_type, activities, indicators_of_compromise, detection_methods.
Generate 12 distinct malware behavior patterns.
  `;

  const behaviors = await synth.generate({
    prompt: malwarePrompt,
    schema: {
      type: 'object',
      properties: {
        behaviors: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              malware_type: {
                type: 'string',
                enum: [
                  'ransomware',
                  'trojan',
                  'rootkit',
                  'keylogger',
                  'backdoor',
                  'worm',
                  'apt_toolkit',
                  'cryptominer'
                ]
              },
              malware_family: { type: 'string' },
              infection_vector: { type: 'string' },
              activities: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    action: { type: 'string' },
                    timestamp_offset: { type: 'number' },
                    process: { type: 'string' },
                    command_line: { type: 'string' },
                    files_accessed: { type: 'array', items: { type: 'string' } },
                    registry_modifications: { type: 'array', items: { type: 'string' } },
                    network_connections: {
                      type: 'array',
                      items: {
                        type: 'object',
                        properties: {
                          destination_ip: { type: 'string' },
                          destination_port: { type: 'number' },
                          protocol: { type: 'string' }
                        }
                      }
                    }
                  }
                }
              },
              indicators_of_compromise: {
                type: 'object',
                properties: {
                  file_hashes: { type: 'array', items: { type: 'string' } },
                  ip_addresses: { type: 'array', items: { type: 'string' } },
                  domains: { type: 'array', items: { type: 'string' } },
                  registry_keys: { type: 'array', items: { type: 'string' } },
                  mutex_names: { type: 'array', items: { type: 'string' } }
                }
              },
              mitre_tactics: { type: 'array', items: { type: 'string' } },
              detection_methods: { type: 'array', items: { type: 'string' } },
              severity: { type: 'string' }
            },
            required: ['id', 'malware_type', 'activities', 'indicators_of_compromise']
          }
        }
      },
      required: ['behaviors']
    }
  });

  return behaviors;
}

/**
 * Phishing Campaign Simulation Data
 * For security awareness training and email filter testing
 */
export async function generatePhishingCampaigns() {
  const synth = new AgenticSynth({
    temperature: 0.8,
    maxRetries: 3
  });

  const phishingPrompt = `
Generate phishing campaign simulations for security awareness training.
Include spear phishing, whaling, vishing, smishing variants.
Each campaign should have: technique, lure, payload, indicators, user_training_points.
Generate 10 diverse phishing campaign scenarios.
  `;

  const campaigns = await synth.generate({
    prompt: phishingPrompt,
    schema: {
      type: 'object',
      properties: {
        campaigns: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              technique: {
                type: 'string',
                enum: [
                  'spear_phishing',
                  'whaling',
                  'clone_phishing',
                  'vishing',
                  'smishing',
                  'angler_phishing',
                  'business_email_compromise'
                ]
              },
              target_audience: { type: 'string' },
              lure_theme: { type: 'string' },
              email_components: {
                type: 'object',
                properties: {
                  subject_line: { type: 'string' },
                  sender_display_name: { type: 'string' },
                  sender_email: { type: 'string' },
                  body_preview: { type: 'string' },
                  call_to_action: { type: 'string' },
                  urgency_level: { type: 'string' }
                }
              },
              payload_type: {
                type: 'string',
                enum: ['credential_harvesting', 'malware_download', 'information_gathering', 'wire_transfer']
              },
              red_flags: { type: 'array', items: { type: 'string' } },
              detection_indicators: { type: 'array', items: { type: 'string' } },
              user_training_points: { type: 'array', items: { type: 'string' } },
              success_metrics: {
                type: 'object',
                properties: {
                  expected_open_rate: { type: 'number' },
                  expected_click_rate: { type: 'number' },
                  expected_report_rate: { type: 'number' }
                }
              },
              severity: { type: 'string' }
            },
            required: ['id', 'technique', 'lure_theme', 'payload_type', 'red_flags']
          }
        }
      },
      required: ['campaigns']
    }
  });

  return campaigns;
}

/**
 * Insider Threat Scenario Simulation
 * For user behavior analytics (UBA) and insider threat detection
 */
export async function generateInsiderThreatScenarios() {
  const synth = new AgenticSynth({
    temperature: 0.7,
    maxRetries: 3
  });

  const insiderPrompt = `
Generate insider threat scenario simulations for security monitoring.
Include data exfiltration, sabotage, privilege abuse, negligent behavior.
Each scenario should have: threat_type, user_profile, activities, anomalies, detection_triggers.
Generate 8 insider threat scenarios.
  `;

  const scenarios = await synth.generate({
    prompt: insiderPrompt,
    schema: {
      type: 'object',
      properties: {
        scenarios: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              threat_type: {
                type: 'string',
                enum: [
                  'data_exfiltration',
                  'intellectual_property_theft',
                  'sabotage',
                  'privilege_abuse',
                  'negligent_behavior',
                  'policy_violation'
                ]
              },
              insider_classification: {
                type: 'string',
                enum: ['malicious', 'negligent', 'compromised']
              },
              user_profile: {
                type: 'object',
                properties: {
                  role: { type: 'string' },
                  access_level: { type: 'string' },
                  tenure_months: { type: 'number' },
                  department: { type: 'string' },
                  baseline_behavior: { type: 'string' }
                }
              },
              timeline: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    day: { type: 'number' },
                    activity: { type: 'string' },
                    anomaly_score: { type: 'number' },
                    data_accessed: { type: 'string' },
                    volume_mb: { type: 'number' }
                  }
                }
              },
              behavioral_anomalies: { type: 'array', items: { type: 'string' } },
              technical_indicators: { type: 'array', items: { type: 'string' } },
              detection_triggers: { type: 'array', items: { type: 'string' } },
              risk_score: { type: 'number' },
              mitigation: { type: 'string' }
            },
            required: ['id', 'threat_type', 'insider_classification', 'user_profile']
          }
        }
      },
      required: ['scenarios']
    }
  });

  return scenarios;
}

/**
 * Zero-Day Exploit Indicator Simulation
 * For testing threat intelligence and anomaly detection systems
 */
export async function generateZeroDayIndicators() {
  const synth = new AgenticSynth({
    temperature: 0.8,
    maxRetries: 3
  });

  const zeroDayPrompt = `
Generate zero-day exploit indicator simulations for threat intelligence testing.
Include unknown malware signatures, unusual network patterns, novel attack techniques.
Each indicator set should have: exploit_target, behavior_patterns, anomaly_indicators, threat_hunting_queries.
Generate 6 zero-day exploit indicator sets.
  `;

  const indicators = await synth.generate({
    prompt: zeroDayPrompt,
    schema: {
      type: 'object',
      properties: {
        indicator_sets: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string' },
              exploit_name: { type: 'string' },
              target_software: { type: 'string' },
              target_version: { type: 'string' },
              vulnerability_type: { type: 'string' },
              behavior_patterns: {
                type: 'array',
                items: {
                  type: 'object',
                  properties: {
                    pattern_type: { type: 'string' },
                    description: { type: 'string' },
                    frequency: { type: 'string' },
                    confidence_level: { type: 'number' }
                  }
                }
              },
              anomaly_indicators: { type: 'array', items: { type: 'string' } },
              network_signatures: { type: 'array', items: { type: 'string' } },
              memory_artifacts: { type: 'array', items: { type: 'string' } },
              threat_hunting_queries: { type: 'array', items: { type: 'string' } },
              detection_difficulty: {
                type: 'string',
                enum: ['low', 'medium', 'high', 'critical']
              },
              potential_impact: { type: 'string' },
              recommended_response: { type: 'string' }
            },
            required: ['id', 'exploit_name', 'target_software', 'behavior_patterns']
          }
        }
      },
      required: ['indicator_sets']
    }
  });

  return indicators;
}

/**
 * Example Usage
 */
export async function runThreatSimulations() {
  console.log('⚠️  Running Authorized Threat Simulations for Defense Testing ⚠️\n');

  try {
    // Generate brute force patterns
    console.log('Generating brute force attack patterns...');
    const bruteForce = await generateBruteForcePatterns();
    console.log(`Generated ${bruteForce.patterns?.length || 0} brute force patterns\n`);

    // Generate DDoS simulations
    console.log('Generating DDoS attack simulations...');
    const ddos = await generateDDoSSimulation();
    console.log(`Generated ${ddos.simulations?.length || 0} DDoS simulations\n`);

    // Generate malware behaviors
    console.log('Generating malware behavior patterns...');
    const malware = await generateMalwareBehaviors();
    console.log(`Generated ${malware.behaviors?.length || 0} malware behaviors\n`);

    // Generate phishing campaigns
    console.log('Generating phishing campaign scenarios...');
    const phishing = await generatePhishingCampaigns();
    console.log(`Generated ${phishing.campaigns?.length || 0} phishing campaigns\n`);

    // Generate insider threat scenarios
    console.log('Generating insider threat scenarios...');
    const insider = await generateInsiderThreatScenarios();
    console.log(`Generated ${insider.scenarios?.length || 0} insider threat scenarios\n`);

    // Generate zero-day indicators
    console.log('Generating zero-day exploit indicators...');
    const zeroDay = await generateZeroDayIndicators();
    console.log(`Generated ${zeroDay.indicator_sets?.length || 0} zero-day indicator sets\n`);

    return {
      bruteForce,
      ddos,
      malware,
      phishing,
      insider,
      zeroDay
    };
  } catch (error) {
    console.error('Error generating threat simulations:', error);
    throw error;
  }
}

// Export all generators
export default {
  generateBruteForcePatterns,
  generateDDoSSimulation,
  generateMalwareBehaviors,
  generatePhishingCampaigns,
  generateInsiderThreatScenarios,
  generateZeroDayIndicators,
  runThreatSimulations
};
