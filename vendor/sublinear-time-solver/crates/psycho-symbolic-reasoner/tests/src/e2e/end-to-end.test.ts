/**
 * End-to-End Tests
 * Tests complete system integration with real agent interactions
 */

import { describe, test, expect, beforeAll, afterAll, beforeEach, afterEach } from '@jest/globals';
import { testUtils } from '@test/test-helpers';
import * as path from 'path';

describe('End-to-End Integration Tests', () => {
  let tempFiles: string[] = [];

  beforeAll(async () => {
    console.log('Setting up E2E test environment...');
  });

  afterAll(async () => {
    // Cleanup temp files
    await testUtils.fsUtils.cleanup(tempFiles);
  });

  beforeEach(() => {
    testUtils.mockAgentFactory.clearAgents();
  });

  describe('Complete Reasoning Pipeline', () => {
    test('should execute full text-to-insight pipeline', async () => {
      const collector = testUtils.performanceCollector;
      collector.start();

      // Scenario: Analyze customer feedback and generate action plan
      const customerFeedback = `
        I've been using your product for 6 months now. Overall, I love the interface design - it's clean and intuitive.
        However, I'm really frustrated with the loading times. Pages take forever to load, especially on mobile.
        I prefer using mobile apps over web versions for daily tasks. The desktop version is okay for heavy work.

        The customer support team is fantastic though! They always respond quickly and are very helpful.
        I'd definitely recommend improving the performance, but I won't switch to competitors because
        I'm emotionally attached to this product. It has become part of my daily routine.

        Price-wise, it's reasonable compared to alternatives. I'm satisfied with the value for money.
        My main concerns are: 1) Mobile performance, 2) Loading speed, 3) Maybe add more integrations.
      `;

      // Step 1: Text Extraction and Analysis
      const extractionAgent = testUtils.mockAgentFactory.createAgent(
        'extractor_001',
        'text_extractor',
        ['sentiment_analysis', 'preference_extraction', 'emotion_detection']
      );

      // Mock text extraction results
      const extractionResults = {
        sentiment: {
          overall_score: 0.3, // Mixed sentiment
          aspects: [
            { aspect: 'interface design', sentiment: 0.8 },
            { aspect: 'loading times', sentiment: -0.9 },
            { aspect: 'customer support', sentiment: 0.9 },
            { aspect: 'price', sentiment: 0.6 }
          ],
          dominant_sentiment: 'mixed'
        },
        preferences: [
          { item: 'mobile apps', category: 'platform', strength: 0.8 },
          { item: 'fast loading', category: 'performance', strength: 0.9 },
          { item: 'desktop for heavy work', category: 'usage', strength: 0.6 }
        ],
        emotions: [
          { type: 'frustration', intensity: 0.7, triggers: ['loading times', 'mobile performance'] },
          { type: 'satisfaction', intensity: 0.8, triggers: ['interface design', 'customer support'] },
          { type: 'attachment', intensity: 0.6, triggers: ['daily routine', 'emotional connection'] }
        ]
      };

      testUtils.mockAgentFactory.updateAgentState('extractor_001', {
        status: 'completed',
        results: extractionResults,
        processed_text_length: customerFeedback.length
      });

      collector.incrementOperation();

      // Step 2: Knowledge Graph Construction
      const reasoningAgent = testUtils.mockAgentFactory.createAgent(
        'reasoner_001',
        'knowledge_reasoner',
        ['graph_construction', 'fact_extraction', 'inference']
      );

      // Extract facts from analysis results
      const knowledgeFacts = [
        { subject: 'user', predicate: 'likes', object: 'interface_design' },
        { subject: 'user', predicate: 'dislikes', object: 'loading_times' },
        { subject: 'user', predicate: 'prefers', object: 'mobile_apps' },
        { subject: 'user', predicate: 'satisfied_with', object: 'customer_support' },
        { subject: 'user', predicate: 'emotionally_attached_to', object: 'product' },
        { subject: 'loading_times', predicate: 'causes', object: 'frustration' },
        { subject: 'mobile_performance', predicate: 'is', object: 'poor' },
        { subject: 'user', predicate: 'wants', object: 'better_performance' }
      ];

      // Mock inference results
      const inferenceResults = [
        { conclusion: 'performance_improvement_critical', confidence: 0.95 },
        { conclusion: 'mobile_optimization_needed', confidence: 0.85 },
        { conclusion: 'user_retention_likely_if_fixed', confidence: 0.8 },
        { conclusion: 'competitive_advantage_in_support', confidence: 0.9 }
      ];

      testUtils.mockAgentFactory.updateAgentState('reasoner_001', {
        status: 'completed',
        knowledge_facts: knowledgeFacts,
        inference_results: inferenceResults,
        graph_stats: { facts: knowledgeFacts.length, entities: 8, relationships: 5 }
      });

      collector.incrementOperation();

      // Step 3: Strategic Planning
      const plannerAgent = testUtils.mockAgentFactory.createAgent(
        'planner_001',
        'strategic_planner',
        ['goal_setting', 'action_planning', 'resource_allocation']
      );

      // Define goals based on insights
      const strategicGoals = [
        {
          id: 'improve_mobile_performance',
          priority: 'high',
          success_criteria: ['mobile_load_time < 2s', 'user_satisfaction > 0.8'],
          estimated_impact: 0.9
        },
        {
          id: 'optimize_loading_speeds',
          priority: 'high',
          success_criteria: ['desktop_load_time < 1s', 'bounce_rate < 10%'],
          estimated_impact: 0.85
        },
        {
          id: 'maintain_support_quality',
          priority: 'medium',
          success_criteria: ['response_time < 2h', 'satisfaction > 0.9'],
          estimated_impact: 0.7
        }
      ];

      // Create action plan
      const actionPlan = {
        immediate_actions: [
          { action: 'audit_mobile_performance', duration: '1 week', cost: 5, priority: 1 },
          { action: 'implement_lazy_loading', duration: '2 weeks', cost: 8, priority: 2 },
          { action: 'optimize_image_compression', duration: '1 week', cost: 3, priority: 3 }
        ],
        medium_term_actions: [
          { action: 'mobile_app_redesign', duration: '8 weeks', cost: 40, priority: 1 },
          { action: 'cdn_implementation', duration: '3 weeks', cost: 15, priority: 2 },
          { action: 'database_optimization', duration: '4 weeks', cost: 20, priority: 3 }
        ],
        long_term_actions: [
          { action: 'progressive_web_app', duration: '12 weeks', cost: 60, priority: 1 },
          { action: 'performance_monitoring_system', duration: '6 weeks', cost: 25, priority: 2 }
        ]
      };

      testUtils.mockAgentFactory.updateAgentState('planner_001', {
        status: 'completed',
        strategic_goals: strategicGoals,
        action_plan: actionPlan,
        total_estimated_cost: 176,
        estimated_completion: '16 weeks'
      });

      collector.incrementOperation();

      // Step 4: Coordination and Validation
      const coordinatorAgent = testUtils.mockAgentFactory.createAgent(
        'coordinator_001',
        'system_coordinator',
        ['workflow_management', 'quality_assurance', 'reporting']
      );

      // Aggregate all results
      const extractorState = testUtils.mockAgentFactory.getAgent('extractor_001')?.state;
      const reasonerState = testUtils.mockAgentFactory.getAgent('reasoner_001')?.state;
      const plannerState = testUtils.mockAgentFactory.getAgent('planner_001')?.state;

      const finalReport = {
        input_analysis: {
          sentiment_score: extractorState?.results.sentiment.overall_score,
          key_preferences: extractorState?.results.preferences.slice(0, 3),
          primary_emotions: extractorState?.results.emotions.slice(0, 2)
        },
        knowledge_insights: {
          critical_facts: reasonerState?.knowledge_facts.slice(0, 5),
          top_inferences: reasonerState?.inference_results.slice(0, 3),
          confidence_level: 0.87
        },
        strategic_recommendations: {
          priority_goals: plannerState?.strategic_goals.filter(g => g.priority === 'high'),
          immediate_actions: plannerState?.action_plan.immediate_actions,
          success_probability: 0.83
        },
        execution_metrics: {
          processing_time: 0, // Will be set by collector
          agents_involved: 4,
          confidence_score: 0.86,
          completeness: 1.0
        }
      };

      testUtils.mockAgentFactory.updateAgentState('coordinator_001', {
        status: 'completed',
        final_report: finalReport,
        workflow_successful: true
      });

      const metrics = collector.stop();
      finalReport.execution_metrics.processing_time = metrics.executionTime;

      // Assertions
      expect(extractorState?.status).toBe('completed');
      expect(reasonerState?.status).toBe('completed');
      expect(plannerState?.status).toBe('completed');

      expect(extractorState?.results.sentiment.aspects).toHaveLength(4);
      expect(reasonerState?.knowledge_facts).toHaveLength(8);
      expect(plannerState?.strategic_goals).toHaveLength(3);

      expect(finalReport.strategic_recommendations.priority_goals).toHaveLength(2);
      expect(finalReport.execution_metrics.agents_involved).toBe(4);
      expect(finalReport.execution_metrics.confidence_score).toBeGreaterThan(0.8);

      console.log('Full pipeline execution time:', metrics.executionTime, 'ms');
      console.log('Final confidence score:', finalReport.execution_metrics.confidence_score);
    });

    test('should handle real-time collaborative reasoning', async () => {
      // Simulate multiple agents working on the same problem simultaneously
      const problem = {
        type: 'optimization_challenge',
        description: 'Find the best route for delivery trucks considering traffic, fuel costs, and time constraints',
        constraints: {
          max_delivery_time: 8, // hours
          fuel_budget: 500, // dollars
          traffic_conditions: 'heavy',
          priority_deliveries: 3
        },
        data: {
          locations: 15,
          delivery_points: 12,
          truck_capacity: 1000, // kg
          current_inventory: 800
        }
      };

      // Create specialized agents
      const agents = [
        testUtils.mockAgentFactory.createAgent('route_optimizer', 'optimizer', ['pathfinding', 'cost_optimization']),
        testUtils.mockAgentFactory.createAgent('traffic_analyzer', 'analyst', ['traffic_analysis', 'time_prediction']),
        testUtils.mockAgentFactory.createAgent('fuel_calculator', 'calculator', ['fuel_estimation', 'cost_analysis']),
        testUtils.mockAgentFactory.createAgent('priority_manager', 'manager', ['priority_scheduling', 'constraint_handling']),
        testUtils.mockAgentFactory.createAgent('integration_coordinator', 'coordinator', ['result_integration', 'decision_making'])
      ];

      // Phase 1: Parallel Analysis
      const analysisPhase = async () => {
        // Route optimization
        testUtils.mockAgentFactory.updateAgentState('route_optimizer', {
          phase: 'analysis',
          status: 'working',
          results: {
            optimal_routes: [
              { route_id: 'R1', distance: 145, estimated_time: 6.5, delivery_points: [1,3,5,7] },
              { route_id: 'R2', distance: 132, estimated_time: 7.2, delivery_points: [2,4,6,8] },
              { route_id: 'R3', distance: 98, estimated_time: 4.8, delivery_points: [9,10,11,12] }
            ],
            optimization_score: 0.87
          }
        });

        // Traffic analysis
        testUtils.mockAgentFactory.updateAgentState('traffic_analyzer', {
          phase: 'analysis',
          status: 'working',
          results: {
            traffic_predictions: [
              { route_id: 'R1', traffic_factor: 1.3, peak_hours: [8,17], congestion_points: 2 },
              { route_id: 'R2', traffic_factor: 1.5, peak_hours: [7,18], congestion_points: 4 },
              { route_id: 'R3', traffic_factor: 1.1, peak_hours: [12], congestion_points: 1 }
            ],
            confidence: 0.82
          }
        });

        // Fuel calculation
        testUtils.mockAgentFactory.updateAgentState('fuel_calculator', {
          phase: 'analysis',
          status: 'working',
          results: {
            fuel_estimates: [
              { route_id: 'R1', fuel_cost: 145, efficiency: 0.85 },
              { route_id: 'R2', fuel_cost: 158, efficiency: 0.78 },
              { route_id: 'R3', fuel_cost: 98, efficiency: 0.92 }
            ],
            total_budget_usage: 0.8
          }
        });

        // Priority management
        testUtils.mockAgentFactory.updateAgentState('priority_manager', {
          phase: 'analysis',
          status: 'working',
          results: {
            priority_assignments: [
              { delivery_id: 1, priority: 'high', time_window: 2, route_preference: 'R1' },
              { delivery_id: 5, priority: 'high', time_window: 3, route_preference: 'R1' },
              { delivery_id: 9, priority: 'high', time_window: 1, route_preference: 'R3' }
            ],
            constraint_satisfaction: 0.91
          }
        });
      };

      await analysisPhase();

      // Phase 2: Integration and Decision Making
      const coordinatorResults = () => {
        const routeOptResults = testUtils.mockAgentFactory.getAgent('route_optimizer')?.state.results;
        const trafficResults = testUtils.mockAgentFactory.getAgent('traffic_analyzer')?.state.results;
        const fuelResults = testUtils.mockAgentFactory.getAgent('fuel_calculator')?.state.results;
        const priorityResults = testUtils.mockAgentFactory.getAgent('priority_manager')?.state.results;

        // Integrate all analyses
        const integratedSolution = {
          recommended_routes: [
            {
              route_id: 'R3',
              reason: 'Best fuel efficiency and lowest traffic',
              score: 0.94,
              deliveries: priorityResults?.priority_assignments.filter(p => p.route_preference === 'R3'),
              metrics: {
                distance: 98,
                fuel_cost: 98,
                estimated_time: 4.8 * 1.1, // with traffic factor
                priority_coverage: 1
              }
            },
            {
              route_id: 'R1',
              reason: 'Covers high priority deliveries efficiently',
              score: 0.88,
              deliveries: priorityResults?.priority_assignments.filter(p => p.route_preference === 'R1'),
              metrics: {
                distance: 145,
                fuel_cost: 145,
                estimated_time: 6.5 * 1.3,
                priority_coverage: 2
              }
            }
          ],
          overall_solution: {
            total_fuel_cost: 243,
            total_time: 13.73,
            priority_deliveries_covered: 3,
            budget_compliance: true,
            time_compliance: true,
            confidence: 0.89
          }
        };

        testUtils.mockAgentFactory.updateAgentState('integration_coordinator', {
          phase: 'integration',
          status: 'completed',
          integrated_solution: integratedSolution,
          decision_rationale: 'Optimal balance of cost, time, and priority constraints'
        });

        return integratedSolution;
      };

      const finalSolution = coordinatorResults();

      // Verify collaborative results
      const allAgents = testUtils.mockAgentFactory.getAllAgents();
      const workingAgents = allAgents.filter(agent =>
        agent.state.phase === 'analysis' || agent.state.phase === 'integration'
      );

      expect(workingAgents).toHaveLength(5);
      expect(finalSolution.recommended_routes).toHaveLength(2);
      expect(finalSolution.overall_solution.budget_compliance).toBe(true);
      expect(finalSolution.overall_solution.time_compliance).toBe(true);
      expect(finalSolution.overall_solution.confidence).toBeGreaterThan(0.85);

      // Verify each agent contributed meaningful results
      expect(testUtils.mockAgentFactory.getAgent('route_optimizer')?.state.results.optimal_routes).toHaveLength(3);
      expect(testUtils.mockAgentFactory.getAgent('traffic_analyzer')?.state.results.traffic_predictions).toHaveLength(3);
      expect(testUtils.mockAgentFactory.getAgent('fuel_calculator')?.state.results.fuel_estimates).toHaveLength(3);
      expect(testUtils.mockAgentFactory.getAgent('priority_manager')?.state.results.priority_assignments).toHaveLength(3);
    });
  });

  describe('Complex Scenario Simulations', () => {
    test('should handle scientific research collaboration', async () => {
      // Scenario: Multiple AI agents collaborating on a research paper
      const researchTopic = {
        title: 'Neural Network Optimization for Real-time Decision Making',
        abstract: 'Investigating novel approaches to optimize neural network inference...',
        keywords: ['neural networks', 'optimization', 'real-time', 'decision making'],
        research_questions: [
          'How can we reduce inference latency by 50%?',
          'What optimization techniques work best for real-time scenarios?',
          'How does optimization affect model accuracy?'
        ]
      };

      // Create research team
      const researchTeam = [
        testUtils.mockAgentFactory.createAgent('literature_reviewer', 'researcher', ['literature_search', 'analysis', 'synthesis']),
        testUtils.mockAgentFactory.createAgent('experiment_designer', 'methodologist', ['experimental_design', 'statistics', 'validation']),
        testUtils.mockAgentFactory.createAgent('data_analyst', 'analyst', ['data_processing', 'statistical_analysis', 'visualization']),
        testUtils.mockAgentFactory.createAgent('technical_writer', 'writer', ['technical_writing', 'documentation', 'peer_review']),
        testUtils.mockAgentFactory.createAgent('research_coordinator', 'coordinator', ['project_management', 'quality_control', 'publication'])
      ];

      // Literature Review Phase
      testUtils.mockAgentFactory.updateAgentState('literature_reviewer', {
        phase: 'literature_review',
        status: 'completed',
        findings: {
          papers_reviewed: 45,
          key_techniques: [
            { technique: 'quantization', effectiveness: 0.8, papers: 12 },
            { technique: 'pruning', effectiveness: 0.75, papers: 15 },
            { technique: 'knowledge_distillation', effectiveness: 0.85, papers: 8 },
            { technique: 'dynamic_inference', effectiveness: 0.9, papers: 6 }
          ],
          research_gaps: [
            'Limited real-time evaluation metrics',
            'Lack of edge device testing',
            'Insufficient accuracy-latency trade-off analysis'
          ],
          confidence: 0.88
        }
      });

      // Experimental Design Phase
      const literatureFindings = testUtils.mockAgentFactory.getAgent('literature_reviewer')?.state.findings;

      testUtils.mockAgentFactory.updateAgentState('experiment_designer', {
        phase: 'experimental_design',
        status: 'completed',
        experimental_plan: {
          hypotheses: [
            'H1: Combined quantization and pruning reduces latency by 60%',
            'H2: Dynamic inference maintains >95% accuracy',
            'H3: Edge deployment feasible with <100ms latency'
          ],
          experiments: [
            {
              id: 'EXP_001',
              name: 'Baseline Performance Measurement',
              duration: '1 week',
              resources: ['GPU cluster', 'benchmark datasets'],
              expected_outcomes: ['baseline_metrics']
            },
            {
              id: 'EXP_002',
              name: 'Optimization Technique Comparison',
              duration: '3 weeks',
              resources: ['GPU cluster', 'optimization frameworks'],
              expected_outcomes: ['technique_effectiveness', 'latency_measurements']
            },
            {
              id: 'EXP_003',
              name: 'Real-time Deployment Testing',
              duration: '2 weeks',
              resources: ['edge devices', 'real-time datasets'],
              expected_outcomes: ['deployment_feasibility', 'accuracy_retention']
            }
          ],
          success_criteria: {
            latency_reduction: 0.5,
            accuracy_retention: 0.95,
            deployment_success: 0.9
          }
        }
      });

      // Data Analysis Phase (simulated experimental results)
      testUtils.mockAgentFactory.updateAgentState('data_analyst', {
        phase: 'data_analysis',
        status: 'completed',
        analysis_results: {
          experimental_data: {
            baseline: { latency: 200, accuracy: 0.94, throughput: 50 },
            quantization: { latency: 120, accuracy: 0.92, throughput: 83 },
            pruning: { latency: 140, accuracy: 0.93, throughput: 71 },
            combined: { latency: 85, accuracy: 0.91, throughput: 118 },
            dynamic: { latency: 95, accuracy: 0.945, throughput: 105 }
          },
          statistical_analysis: {
            significant_improvements: ['latency', 'throughput'],
            accuracy_trade_offs: 'minimal (<3%)',
            confidence_intervals: {
              latency_reduction: [0.52, 0.67],
              accuracy_retention: [0.91, 0.95]
            }
          },
          visualizations: [
            'latency_comparison_chart',
            'accuracy_scatter_plot',
            'pareto_frontier_analysis'
          ]
        }
      });

      // Technical Writing Phase
      const experimentalResults = testUtils.mockAgentFactory.getAgent('data_analyst')?.state.analysis_results;

      testUtils.mockAgentFactory.updateAgentState('technical_writer', {
        phase: 'writing',
        status: 'completed',
        manuscript: {
          sections: {
            abstract: { word_count: 250, quality_score: 0.9 },
            introduction: { word_count: 800, quality_score: 0.88 },
            methodology: { word_count: 1200, quality_score: 0.92 },
            results: { word_count: 1000, quality_score: 0.95 },
            discussion: { word_count: 900, quality_score: 0.87 },
            conclusion: { word_count: 400, quality_score: 0.91 }
          },
          total_word_count: 4550,
          figures: 8,
          tables: 3,
          references: 47,
          readability_score: 0.84,
          technical_accuracy: 0.93
        }
      });

      // Research Coordination and Quality Control
      testUtils.mockAgentFactory.updateAgentState('research_coordinator', {
        phase: 'coordination',
        status: 'completed',
        project_summary: {
          objectives_met: {
            latency_reduction: 0.575, // Exceeded 50% target
            accuracy_retention: 0.945, // Met 95% target
            deployment_feasibility: 0.92 // Exceeded 90% target
          },
          research_contributions: [
            'Novel combined optimization approach',
            'Real-time evaluation framework',
            'Edge deployment methodology'
          ],
          publication_readiness: {
            scientific_rigor: 0.91,
            novelty: 0.87,
            practical_impact: 0.93,
            reproducibility: 0.89
          },
          next_steps: [
            'Submit to top-tier conference',
            'Open-source implementation',
            'Industry collaboration'
          ]
        }
      });

      // Verify research collaboration success
      const teamMembers = testUtils.mockAgentFactory.getAllAgents();
      const completedResearchers = teamMembers.filter(agent =>
        agent.state.status === 'completed'
      );

      expect(completedResearchers).toHaveLength(5);

      const literatureReview = testUtils.mockAgentFactory.getAgent('literature_reviewer')?.state.findings;
      const experimentPlan = testUtils.mockAgentFactory.getAgent('experiment_designer')?.state.experimental_plan;
      const dataAnalysis = testUtils.mockAgentFactory.getAgent('data_analyst')?.state.analysis_results;
      const manuscript = testUtils.mockAgentFactory.getAgent('technical_writer')?.state.manuscript;
      const projectSummary = testUtils.mockAgentFactory.getAgent('research_coordinator')?.state.project_summary;

      // Verify research quality
      expect(literatureReview?.papers_reviewed).toBe(45);
      expect(experimentPlan?.experiments).toHaveLength(3);
      expect(dataAnalysis?.experimental_data.combined.latency).toBeLessThan(100);
      expect(manuscript?.total_word_count).toBeGreaterThan(4000);
      expect(projectSummary?.objectives_met.latency_reduction).toBeGreaterThan(0.5);
      expect(projectSummary?.publication_readiness.scientific_rigor).toBeGreaterThan(0.9);
    });

    test('should simulate crisis response coordination', async () => {
      // Scenario: Emergency response system with multiple AI agents
      const emergencyEvent = {
        type: 'natural_disaster',
        event: 'flood',
        location: { lat: 40.7128, lng: -74.0060, city: 'New York' },
        severity: 'high',
        affected_population: 50000,
        infrastructure_damage: ['roads', 'power_grid', 'communication'],
        timestamp: Date.now(),
        weather_conditions: 'heavy_rain_continuing'
      };

      // Create emergency response team
      const emergencyTeam = [
        testUtils.mockAgentFactory.createAgent('situation_assessor', 'assessor', ['damage_assessment', 'risk_analysis', 'resource_estimation']),
        testUtils.mockAgentFactory.createAgent('resource_coordinator', 'coordinator', ['resource_allocation', 'logistics', 'priority_management']),
        testUtils.mockAgentFactory.createAgent('evacuation_planner', 'planner', ['route_planning', 'capacity_management', 'safety_protocols']),
        testUtils.mockAgentFactory.createAgent('communication_manager', 'communicator', ['public_communication', 'media_relations', 'alert_systems']),
        testUtils.mockAgentFactory.createAgent('incident_commander', 'commander', ['decision_making', 'coordination', 'strategic_oversight'])
      ];

      // Immediate Assessment Phase (0-30 minutes)
      testUtils.mockAgentFactory.updateAgentState('situation_assessor', {
        phase: 'immediate_assessment',
        status: 'completed',
        assessment: {
          damage_analysis: {
            infrastructure: {
              roads: { affected_percentage: 0.3, critical_routes: 8, estimated_repair_time: '72 hours' },
              power_grid: { affected_percentage: 0.45, outages: 15000, restoration_priority: 'high' },
              communication: { affected_percentage: 0.2, cell_towers_down: 5, backup_systems: 'active' }
            },
            casualties: {
              estimated_injured: 25,
              missing_persons: 12,
              evacuees: 8500,
              medical_facilities_operational: 0.8
            }
          },
          risk_factors: {
            continued_flooding: 0.7,
            structural_collapse: 0.3,
            disease_outbreak: 0.15,
            supply_chain_disruption: 0.6
          },
          urgency_level: 'critical'
        }
      });

      // Resource Coordination Phase (15-45 minutes)
      const assessmentData = testUtils.mockAgentFactory.getAgent('situation_assessor')?.state.assessment;

      testUtils.mockAgentFactory.updateAgentState('resource_coordinator', {
        phase: 'resource_coordination',
        status: 'completed',
        resource_allocation: {
          emergency_services: {
            ambulances: { deployed: 15, en_route: 8, eta_average: 12 },
            fire_trucks: { deployed: 10, en_route: 5, eta_average: 8 },
            police_units: { deployed: 25, en_route: 12, eta_average: 6 },
            rescue_boats: { deployed: 8, en_route: 4, eta_average: 15 }
          },
          supplies: {
            medical_supplies: { allocated: 85, distribution_points: 6 },
            food_water: { allocated: 90, distribution_points: 8 },
            shelter_materials: { allocated: 70, distribution_points: 4 },
            communication_equipment: { allocated: 95, distribution_points: 3 }
          },
          personnel: {
            first_responders: 150,
            medical_staff: 45,
            volunteers: 200,
            coordination_staff: 25
          },
          allocation_efficiency: 0.87
        }
      });

      // Evacuation Planning Phase (20-60 minutes)
      testUtils.mockAgentFactory.updateAgentState('evacuation_planner', {
        phase: 'evacuation_planning',
        status: 'completed',
        evacuation_plan: {
          evacuation_zones: [
            { zone_id: 'A', population: 15000, risk_level: 'extreme', evacuation_order: 'mandatory' },
            { zone_id: 'B', population: 20000, risk_level: 'high', evacuation_order: 'recommended' },
            { zone_id: 'C', population: 15000, risk_level: 'moderate', evacuation_order: 'voluntary' }
          ],
          routes: [
            { route_id: 'R1', capacity: 5000, current_load: 0.3, estimated_time: 45 },
            { route_id: 'R2', capacity: 7000, current_load: 0.6, estimated_time: 60 },
            { route_id: 'R3', capacity: 4000, current_load: 0.2, estimated_time: 35 }
          ],
          shelters: [
            { shelter_id: 'S1', capacity: 2000, current_occupancy: 450, amenities: ['medical', 'food'] },
            { shelter_id: 'S2', capacity: 3000, current_occupancy: 1200, amenities: ['food', 'communication'] },
            { shelter_id: 'S3', capacity: 1500, current_occupancy: 300, amenities: ['medical', 'pet_friendly'] }
          ],
          timeline: {
            zone_a_completion: 90, // minutes
            zone_b_completion: 180,
            zone_c_completion: 240,
            total_evacuation_time: 240
          }
        }
      });

      // Communication Management Phase (0-ongoing)
      testUtils.mockAgentFactory.updateAgentState('communication_manager', {
        phase: 'communication_management',
        status: 'active',
        communication_strategy: {
          public_alerts: {
            emergency_broadcast: { sent: true, reach: 0.85, response_rate: 0.7 },
            social_media: { platforms: 4, reach: 0.92, engagement: 0.68 },
            mobile_alerts: { sent: true, delivery_rate: 0.78, acknowledgment_rate: 0.6 }
          },
          media_coordination: {
            press_releases: 3,
            media_briefings_scheduled: 2,
            spokesperson_assigned: true,
            message_consistency: 0.94
          },
          information_updates: {
            situation_reports: 5,
            evacuation_instructions: 8,
            resource_information: 12,
            weather_updates: 15
          },
          feedback_channels: {
            emergency_hotline: { calls_received: 450, response_rate: 0.88 },
            social_media_monitoring: { mentions_tracked: 1200, sentiment: 0.3 },
            community_liaisons: { active: 8, coverage: 0.75 }
          }
        }
      });

      // Incident Command Coordination (ongoing)
      testUtils.mockAgentFactory.updateAgentState('incident_commander', {
        phase: 'incident_command',
        status: 'active',
        command_decisions: {
          strategic_priorities: [
            { priority: 'life_safety', status: 'ongoing', progress: 0.8 },
            { priority: 'property_protection', status: 'ongoing', progress: 0.6 },
            { priority: 'environmental_protection', status: 'pending', progress: 0.2 }
          ],
          resource_requests: [
            { request: 'additional_medical_teams', status: 'approved', eta: 45 },
            { request: 'heavy_rescue_equipment', status: 'en_route', eta: 75 },
            { request: 'temporary_bridges', status: 'approved', eta: 180 }
          ],
          operational_periods: [
            { period: 1, duration: 120, objectives: ['immediate_rescue', 'evacuation_initiation'] },
            { period: 2, duration: 240, objectives: ['mass_evacuation', 'shelter_operations'] },
            { period: 3, duration: 360, objectives: ['damage_assessment', 'recovery_planning'] }
          ],
          coordination_effectiveness: 0.89,
          decision_confidence: 0.91
        }
      });

      // Verify emergency response coordination
      const responseTeam = testUtils.mockAgentFactory.getAllAgents();
      const activeResponders = responseTeam.filter(agent =>
        agent.state.status === 'completed' || agent.state.status === 'active'
      );

      expect(activeResponders).toHaveLength(5);

      const assessment = testUtils.mockAgentFactory.getAgent('situation_assessor')?.state.assessment;
      const resources = testUtils.mockAgentFactory.getAgent('resource_coordinator')?.state.resource_allocation;
      const evacuation = testUtils.mockAgentFactory.getAgent('evacuation_planner')?.state.evacuation_plan;
      const communication = testUtils.mockAgentFactory.getAgent('communication_manager')?.state.communication_strategy;
      const command = testUtils.mockAgentFactory.getAgent('incident_commander')?.state.command_decisions;

      // Verify response effectiveness
      expect(assessment?.urgency_level).toBe('critical');
      expect(resources?.allocation_efficiency).toBeGreaterThan(0.8);
      expect(evacuation?.timeline.total_evacuation_time).toBeLessThan(300);
      expect(communication?.public_alerts.emergency_broadcast.reach).toBeGreaterThan(0.8);
      expect(command?.coordination_effectiveness).toBeGreaterThan(0.85);

      // Verify critical metrics
      expect(evacuation?.evacuation_zones[0].evacuation_order).toBe('mandatory');
      expect(resources?.emergency_services.ambulances.deployed).toBeGreaterThan(10);
      expect(communication?.information_updates.evacuation_instructions).toBeGreaterThan(5);
      expect(command?.strategic_priorities[0].progress).toBeGreaterThan(0.7);
    });
  });

  describe('Performance and Scalability', () => {
    test('should handle large-scale agent networks', async () => {
      const networkSize = 100;
      const agents = [];

      // Create hierarchical agent network
      for (let i = 0; i < networkSize; i++) {
        const agentType = i < 10 ? 'coordinator' :
                         i < 30 ? 'specialist' :
                         i < 60 ? 'worker' : 'monitor';

        const agent = testUtils.mockAgentFactory.createAgent(
          `agent_${i}`,
          agentType,
          [`capability_${i % 10}`, `skill_${i % 15}`]
        );
        agents.push(agent);
      }

      // Simulate complex task distribution
      const taskDistribution = async () => {
        const coordinators = agents.filter(a => a.type === 'coordinator');
        const workers = agents.filter(a => a.type === 'worker');

        // Each coordinator manages multiple workers
        for (const coord of coordinators) {
          const managedWorkers = workers.slice(
            parseInt(coord.id.split('_')[1]) * 3,
            (parseInt(coord.id.split('_')[1]) + 1) * 3
          );

          for (const worker of managedWorkers) {
            testUtils.mockAgentFactory.sendMessage(worker.id, {
              from: coord.id,
              type: 'task_assignment',
              payload: {
                task_id: `task_${Date.now()}_${Math.random()}`,
                complexity: Math.random(),
                deadline: Date.now() + 300000 // 5 minutes
              }
            });
          }

          testUtils.mockAgentFactory.updateAgentState(coord.id, {
            managed_workers: managedWorkers.length,
            tasks_assigned: managedWorkers.length,
            status: 'coordinating'
          });
        }
      };

      const collector = testUtils.performanceCollector;
      collector.start();

      await taskDistribution();

      // Process all messages
      const messages = testUtils.mockAgentFactory.processMessages();
      const metrics = collector.stop();

      expect(agents).toHaveLength(networkSize);
      expect(messages.length).toBeGreaterThan(20); // At least some coordination happened
      expect(metrics.executionTime).toBeLessThan(5000); // Under 5 seconds

      const coordinators = testUtils.mockAgentFactory.getAllAgents()
        .filter(a => a.type === 'coordinator');

      expect(coordinators.every(c => c.state.status === 'coordinating')).toBe(true);

      console.log(`Managed ${networkSize} agents with ${messages.length} messages in ${metrics.executionTime}ms`);
    });

    test('should maintain performance under stress', async () => {
      const stressTestDuration = 30000; // 30 seconds
      const operationInterval = 100; // 100ms between operations
      const expectedOperations = stressTestDuration / operationInterval;

      const detector = testUtils.memoryLeakDetector;
      detector.start();

      const stressTest = async () => {
        const startTime = Date.now();
        let operationCount = 0;

        while (Date.now() - startTime < stressTestDuration) {
          // Create temporary agents
          const agent = testUtils.mockAgentFactory.createAgent(
            `stress_agent_${operationCount}`,
            'stress_tester',
            ['high_frequency_ops']
          );

          // Perform operations
          testUtils.mockAgentFactory.sendMessage(agent.id, {
            from: 'stress_coordinator',
            type: 'stress_test',
            payload: { operation_id: operationCount }
          });

          testUtils.mockAgentFactory.updateAgentState(agent.id, {
            operation_count: operationCount,
            timestamp: Date.now()
          });

          operationCount++;

          // Take memory snapshots periodically
          if (operationCount % 50 === 0) {
            detector.snapshot();
          }

          await testUtils.asyncUtils.sleep(operationInterval);
        }

        return operationCount;
      };

      const actualOperations = await stressTest();
      const leakCheck = detector.checkForLeaks();

      expect(actualOperations).toBeGreaterThan(expectedOperations * 0.8); // Allow some variance
      expect(leakCheck.hasLeak).toBe(false);
      expect(leakCheck.memoryIncrease).toBeLessThan(50 * 1024 * 1024); // Less than 50MB

      console.log(`Stress test: ${actualOperations} operations, memory increase: ${Math.round(leakCheck.memoryIncrease / 1024)}KB`);
    });
  });
});