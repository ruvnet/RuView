/**
 * Organizational Dynamics Simulation
 *
 * Generates realistic team formation, cross-functional collaboration,
 * leadership effectiveness, mentorship relationships, and cultural indicators
 * for organizational planning and analysis.
 *
 * ETHICAL USE: These simulations model organizational behavior patterns.
 * Always maintain confidentiality and use only for legitimate org planning.
 */

import { createSynth } from '../../src/index.js';

/**
 * Generate team formation and evolution data
 * Models how teams form, grow, and change over time
 */
export async function generateTeamDynamics() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const teamSchema = {
    teamId: { type: 'string', required: true },
    teamName: { type: 'string', required: true },
    department: { type: 'string', required: true },
    formationDate: { type: 'string', required: true },
    size: { type: 'number', required: true },
    composition: {
      type: 'object',
      required: true,
      properties: {
        senior: { type: 'number' },
        mid: { type: 'number' },
        junior: { type: 'number' },
        manager: { type: 'number' }
      }
    },
    diversity: {
      type: 'object',
      required: true,
      properties: {
        genderBalance: { type: 'number' }, // percentage women
        ageRange: { type: 'string' },
        tenureSpread: { type: 'number' }, // years variance
        skillDiversity: { type: 'number' } // 0-100
      }
    },
    performance: {
      type: 'object',
      required: true,
      properties: {
        velocity: { type: 'number' },
        qualityScore: { type: 'number' },
        collaborationScore: { type: 'number' },
        innovationScore: { type: 'number' }
      }
    },
    stage: {
      type: 'string',
      required: true,
      enum: ['forming', 'storming', 'norming', 'performing', 'adjourning']
    },
    healthScore: {
      type: 'number',
      required: true,
      min: 0,
      max: 100
    },
    turnoverRate: { type: 'number', required: true }, // percentage annual
    remoteMembers: { type: 'number', required: true }
  };

  const result = await synth.generateStructured({
    count: 50,
    schema: teamSchema,
    format: 'json',
    context: `Generate realistic team dynamics:
    - Team size: 5-12 members (optimal: 7-9)
    - Composition: 1-2 senior, 3-5 mid, 1-3 junior, 1 manager
    - Gender balance: 30-70% (increasing trend)
    - Age range: 22-65, modal 28-35
    - Stage progression: 3 months forming, 2 months storming, then performing
    - High performing teams: >85 health score, <10% turnover
    - Struggling teams: <60 health score, >25% turnover
    - Remote ratio: 20-60%
    - Diversity correlates with innovation (+15-20%)`
  });

  return result;
}

/**
 * Generate cross-functional collaboration data
 * Models interactions between departments and teams
 */
export async function generateCrossFunctionalCollaboration() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const collaborationSchema = {
    initiativeId: { type: 'string', required: true },
    initiativeName: { type: 'string', required: true },
    startDate: { type: 'string', required: true },
    teamsInvolved: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          teamId: { type: 'string' },
          department: { type: 'string' },
          memberCount: { type: 'number' },
          contributionLevel: { type: 'string', enum: ['lead', 'major', 'minor', 'support'] }
        }
      }
    },
    collaborationMetrics: {
      type: 'object',
      required: true,
      properties: {
        meetingFrequency: { type: 'number' }, // per week
        communicationScore: { type: 'number' }, // 0-100
        alignmentScore: { type: 'number' }, // 0-100
        conflictLevel: { type: 'string', enum: ['none', 'low', 'moderate', 'high'] }
      }
    },
    outcomes: {
      type: 'object',
      required: true,
      properties: {
        delivered: { type: 'boolean' },
        onTime: { type: 'boolean' },
        budgetAdherence: { type: 'number' }, // percentage
        stakeholderSatisfaction: { type: 'number' }, // 1-10
        innovationScore: { type: 'number' } // 1-10
      }
    },
    barriers: {
      type: 'array',
      required: true,
      items: {
        type: 'string',
        enum: ['communication', 'priorities', 'resources', 'tools', 'culture', 'process']
      }
    },
    successFactors: {
      type: 'array',
      required: true,
      items: { type: 'string' }
    }
  };

  const result = await synth.generateStructured({
    count: 100,
    schema: collaborationSchema,
    format: 'json',
    context: `Generate cross-functional collaboration patterns:
    - 2-5 teams per initiative (sweet spot: 3)
    - Delivery rate: 75% delivered, 60% on-time
    - Budget: 80% within ±10%
    - More teams = lower alignment, higher conflict
    - Success factors: clear goals, executive sponsor, dedicated time
    - Common barriers: competing priorities (40%), communication (30%), resources (20%)
    - High communication score correlates with success
    - Innovation higher with 3+ teams (+25%)
    - Include both successful and challenging collaborations`
  });

  return result;
}

/**
 * Generate leadership effectiveness data
 * Models manager and leadership impact on teams
 */
export async function generateLeadershipEffectiveness() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const leadershipSchema = {
    leaderId: { type: 'string', required: true },
    role: {
      type: 'string',
      required: true,
      enum: ['team_lead', 'manager', 'director', 'vp', 'executive']
    },
    tenureInRole: { type: 'number', required: true }, // months
    teamSize: { type: 'number', required: true },
    directReports: { type: 'number', required: true },
    leadershipMetrics: {
      type: 'object',
      required: true,
      properties: {
        teamEngagement: { type: 'number', min: 0, max: 100 },
        teamRetention: { type: 'number', min: 0, max: 100 },
        teamPerformance: { type: 'number', min: 0, max: 100 },
        oneOnOneFrequency: { type: 'number' }, // per month
        feedbackQuality: { type: 'number', min: 1, max: 5 }
      }
    },
    competencies: {
      type: 'object',
      required: true,
      properties: {
        strategicThinking: { type: 'number', min: 1, max: 5 },
        peopleManagement: { type: 'number', min: 1, max: 5 },
        communication: { type: 'number', min: 1, max: 5 },
        decisionMaking: { type: 'number', min: 1, max: 5 },
        emotionalIntelligence: { type: 'number', min: 1, max: 5 }
      }
    },
    upwardFeedback: {
      type: 'object',
      required: true,
      properties: {
        participationRate: { type: 'number' }, // percentage
        overallScore: { type: 'number', min: 1, max: 5 },
        recommendationRate: { type: 'number' } // percentage who would recommend
      }
    },
    developmentAreas: {
      type: 'array',
      required: true,
      items: { type: 'string' }
    },
    successorReadiness: {
      type: 'string',
      required: true,
      enum: ['immediate', 'within_year', 'within_two_years', 'not_identified']
    }
  };

  const result = await synth.generateStructured({
    count: 80,
    schema: leadershipSchema,
    format: 'json',
    context: `Generate leadership effectiveness data:
    - Team engagement: mean 72%, stddev 15%
    - Retention: 85% average (range 60-95%)
    - High performers: >4.0 all competencies, >80% recommendation rate
    - Direct reports: Team lead 5-8, Manager 8-12, Director 15-30
    - 1:1 frequency: 2-4 per month (higher for new reports)
    - Correlation: EQ score strongly predicts retention (+20% at 5.0 vs 3.0)
    - New leaders (0-6 months): lower scores, higher variability
    - Experienced leaders: more consistent, higher scores
    - Successor ready: 40% immediate/within year
    - Include spectrum from struggling to exceptional leaders`
  });

  return result;
}

/**
 * Generate mentorship relationship data
 * Models mentor-mentee pairings and outcomes
 */
export async function generateMentorshipData() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const mentorshipSchema = {
    relationshipId: { type: 'string', required: true },
    mentorId: { type: 'string', required: true },
    menteeId: { type: 'string', required: true },
    startDate: { type: 'string', required: true },
    status: {
      type: 'string',
      required: true,
      enum: ['active', 'completed', 'paused', 'ended_early']
    },
    mentorProfile: {
      type: 'object',
      required: true,
      properties: {
        yearsExperience: { type: 'number' },
        department: { type: 'string' },
        level: { type: 'string' }
      }
    },
    menteeProfile: {
      type: 'object',
      required: true,
      properties: {
        yearsExperience: { type: 'number' },
        department: { type: 'string' },
        level: { type: 'string' }
      }
    },
    engagement: {
      type: 'object',
      required: true,
      properties: {
        meetingsHeld: { type: 'number' },
        meetingsPlanned: { type: 'number' },
        avgMeetingDuration: { type: 'number' }, // minutes
        communicationFrequency: { type: 'string', enum: ['weekly', 'biweekly', 'monthly'] }
      }
    },
    focusAreas: {
      type: 'array',
      required: true,
      items: {
        type: 'string',
        enum: ['career_development', 'technical_skills', 'leadership', 'networking', 'work_life_balance', 'specific_project']
      }
    },
    outcomes: {
      type: 'object',
      required: true,
      properties: {
        menteeSatisfaction: { type: 'number', min: 1, max: 5 },
        mentorSatisfaction: { type: 'number', min: 1, max: 5 },
        goalsAchieved: { type: 'number' }, // percentage
        skillsGained: { type: 'number' },
        networkExpanded: { type: 'boolean' }
      }
    },
    impact: {
      type: 'object',
      required: true,
      properties: {
        promotionWithinYear: { type: 'boolean' },
        retentionImproved: { type: 'boolean' },
        performanceImprovement: { type: 'number' } // percentage points
      }
    }
  };

  const result = await synth.generateStructured({
    count: 150,
    schema: mentorshipSchema,
    format: 'json',
    context: `Generate mentorship relationship data:
    - Active: 70%, Completed: 20%, Ended early: 10%
    - Meeting attendance: 80% of planned
    - Duration: 45-60 minutes average
    - Satisfaction: mean 4.2/5 (mentees), 4.0/5 (mentors)
    - 75% achieve 70%+ of goals
    - Cross-departmental mentoring: 40% of relationships
    - Same department: higher technical skill transfer
    - Different department: better networking outcomes
    - Promotion impact: 25% promoted within year (vs 15% baseline)
    - Retention: 10% improvement for mentees
    - Include diverse pairings and outcomes`
  });

  return result;
}

/**
 * Generate organizational culture indicators
 * Models cultural health and employee sentiment
 */
export async function generateCultureIndicators() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const cultureSchema = {
    surveyId: { type: 'string', required: true },
    department: { type: 'string', required: true },
    surveyDate: { type: 'string', required: true },
    participationRate: { type: 'number', required: true }, // percentage
    dimensions: {
      type: 'object',
      required: true,
      properties: {
        trust: { type: 'number', min: 0, max: 100 },
        transparency: { type: 'number', min: 0, max: 100 },
        innovation: { type: 'number', min: 0, max: 100 },
        collaboration: { type: 'number', min: 0, max: 100 },
        workLifeBalance: { type: 'number', min: 0, max: 100 },
        diversity: { type: 'number', min: 0, max: 100 },
        growth: { type: 'number', min: 0, max: 100 },
        recognition: { type: 'number', min: 0, max: 100 }
      }
    },
    engagement: {
      type: 'object',
      required: true,
      properties: {
        overallScore: { type: 'number', min: 0, max: 100 },
        eNPS: { type: 'number', min: -100, max: 100 },
        recommendRate: { type: 'number' } // percentage
      }
    },
    sentimentAnalysis: {
      type: 'object',
      required: true,
      properties: {
        positive: { type: 'number' }, // percentage
        neutral: { type: 'number' },
        negative: { type: 'number' }
      }
    },
    topThemes: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          theme: { type: 'string' },
          sentiment: { type: 'string', enum: ['positive', 'neutral', 'negative'] },
          frequency: { type: 'number' }
        }
      }
    },
    actionItems: {
      type: 'array',
      required: true,
      items: { type: 'string' }
    }
  };

  const result = await synth.generateStructured({
    count: 40,
    schema: cultureSchema,
    format: 'json',
    context: `Generate culture survey data:
    - Participation: 65-85% (higher in engaged orgs)
    - Overall engagement: mean 72%, stddev 12%
    - eNPS: mean +25, range -20 to +60
    - Dimension scores: typically 65-85 range
    - Variations by department: Engineering +5%, Sales -3%
    - Remote teams: +10% work-life balance, -5% collaboration
    - Sentiment: 55% positive, 30% neutral, 15% negative
    - Common positive themes: flexibility, growth, team
    - Common negative themes: processes, communication, resources
    - Correlations: trust predicts engagement, innovation follows growth`
  });

  return result;
}

/**
 * Generate succession planning scenarios
 * Models leadership pipeline and readiness
 */
export async function generateSuccessionPlanning() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const successionSchema = {
    positionId: { type: 'string', required: true },
    positionTitle: { type: 'string', required: true },
    level: {
      type: 'string',
      required: true,
      enum: ['manager', 'senior_manager', 'director', 'vp', 'svp', 'c_suite']
    },
    criticality: {
      type: 'string',
      required: true,
      enum: ['critical', 'important', 'standard']
    },
    currentHolder: {
      type: 'object',
      required: true,
      properties: {
        employeeId: { type: 'string' },
        tenure: { type: 'number' }, // years
        retirementRisk: { type: 'string', enum: ['0-2_years', '2-5_years', '5+_years'] },
        flightRisk: { type: 'string', enum: ['low', 'medium', 'high'] }
      }
    },
    successors: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          employeeId: { type: 'string' },
          readiness: { type: 'string', enum: ['ready_now', '1_year', '2_years', '3+_years'] },
          gapAnalysis: {
            type: 'array',
            items: { type: 'string' }
          },
          potentialScore: { type: 'number', min: 1, max: 5 },
          performanceScore: { type: 'number', min: 1, max: 5 }
        }
      }
    },
    developmentPlan: {
      type: 'object',
      required: true,
      properties: {
        exists: { type: 'boolean' },
        lastUpdated: { type: 'string' },
        keyActions: {
          type: 'array',
          items: { type: 'string' }
        }
      }
    },
    riskLevel: {
      type: 'string',
      required: true,
      enum: ['low', 'medium', 'high', 'critical']
    }
  };

  const result = await synth.generateStructured({
    count: 60,
    schema: successionSchema,
    format: 'json',
    context: `Generate succession planning data:
    - Critical positions: 30%, Important: 50%, Standard: 20%
    - Ready now successors: 25% of positions
    - 1-2 year ready: 45% of positions
    - No identified successor: 15% of positions (high risk)
    - Average 1.8 successors per position
    - 9-box model: High potential + high performance = ready now
    - Common gaps: strategic thinking, executive presence, financial acumen
    - Development plans exist for 70% of critical roles
    - Flight risk: increases without clear path (+30% turnover)
    - Retirement pipeline: 15% of leaders within 5 years
    - Include diversity in succession pipeline`
  });

  return result;
}

/**
 * Run all organizational dynamics examples
 */
export async function runAllOrganizationalExamples() {
  console.log('=== Organizational Dynamics Simulation Examples ===\n');

  console.log('1. Generating Team Dynamics...');
  const teams = await generateTeamDynamics();
  console.log(`Generated ${teams.data.length} team records`);
  console.log('Sample:', JSON.stringify(teams.data[0], null, 2));

  console.log('\n2. Generating Cross-Functional Collaboration...');
  const collaboration = await generateCrossFunctionalCollaboration();
  console.log(`Generated ${collaboration.data.length} collaboration records`);
  console.log('Sample:', JSON.stringify(collaboration.data[0], null, 2));

  console.log('\n3. Generating Leadership Effectiveness...');
  const leadership = await generateLeadershipEffectiveness();
  console.log(`Generated ${leadership.data.length} leadership records`);
  console.log('Sample:', JSON.stringify(leadership.data[0], null, 2));

  console.log('\n4. Generating Mentorship Data...');
  const mentorship = await generateMentorshipData();
  console.log(`Generated ${mentorship.data.length} mentorship records`);
  console.log('Sample:', JSON.stringify(mentorship.data[0], null, 2));

  console.log('\n5. Generating Culture Indicators...');
  const culture = await generateCultureIndicators();
  console.log(`Generated ${culture.data.length} culture survey records`);
  console.log('Sample:', JSON.stringify(culture.data[0], null, 2));

  console.log('\n6. Generating Succession Planning...');
  const succession = await generateSuccessionPlanning();
  console.log(`Generated ${succession.data.length} succession planning records`);
  console.log('Sample:', JSON.stringify(succession.data[0], null, 2));
}

// Uncomment to run
// runAllOrganizationalExamples().catch(console.error);
