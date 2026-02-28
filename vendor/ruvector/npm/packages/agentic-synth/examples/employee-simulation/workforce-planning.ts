/**
 * Workforce Planning Data Simulation
 *
 * Generates realistic hiring forecasts, skill gap analysis, turnover predictions,
 * compensation data, career paths, and diversity metrics for strategic HR planning.
 *
 * PRIVACY & ETHICS: This data is synthetic for planning purposes only.
 * Never use for actual hiring decisions or to bias against protected groups.
 */

import { createSynth } from '../../src/index.js';

/**
 * Generate hiring needs forecasting data
 * Models future workforce requirements based on growth and attrition
 */
export async function generateHiringForecast() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const forecastSchema = {
    forecastPeriod: { type: 'string', required: true },
    department: { type: 'string', required: true },
    currentHeadcount: { type: 'number', required: true },
    projectedGrowth: { type: 'number', required: true }, // percentage
    expectedAttrition: { type: 'number', required: true }, // percentage
    hiringNeeds: {
      type: 'object',
      required: true,
      properties: {
        newRoles: { type: 'number' },
        backfills: { type: 'number' },
        total: { type: 'number' }
      }
    },
    roleBreakdown: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          role: { type: 'string' },
          level: { type: 'string', enum: ['junior', 'mid', 'senior', 'lead', 'principal'] },
          count: { type: 'number' },
          priority: { type: 'string', enum: ['critical', 'high', 'medium', 'low'] },
          timeToFill: { type: 'number' }, // days
          difficulty: { type: 'string', enum: ['easy', 'moderate', 'challenging', 'very_challenging'] }
        }
      }
    },
    budgetRequired: { type: 'number', required: true },
    talentAvailability: {
      type: 'string',
      required: true,
      enum: ['abundant', 'adequate', 'limited', 'scarce']
    },
    competingEmployers: { type: 'number', required: true },
    risks: {
      type: 'array',
      required: true,
      items: { type: 'string' }
    }
  };

  const result = await synth.generateStructured({
    count: 40,
    schema: forecastSchema,
    format: 'json',
    context: `Generate hiring forecast data:
    - Growth: 10-30% annual for growth companies, 0-10% for mature
    - Attrition: 12-18% average (tech industry)
    - Senior roles: 60-90 days to fill, very challenging
    - Junior roles: 30-45 days to fill, moderate difficulty
    - Critical roles: ML engineers, cybersecurity, senior backend
    - Budget: $100K-$180K per engineer, $80K-$130K per designer
    - Talent scarcity: ML/AI scarce, frontend moderate, support abundant
    - Risks: competing offers, remote work expectations, skill gaps
    - Seasonal patterns: Q1 high, Q3 low
    - Include headcount changes month-by-month`
  });

  return result;
}

/**
 * Generate skill gap analysis data
 * Identifies current vs required skills for strategic planning
 */
export async function generateSkillGapAnalysis() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const skillGapSchema = {
    analysisDate: { type: 'string', required: true },
    department: { type: 'string', required: true },
    skillCategory: {
      type: 'string',
      required: true,
      enum: ['technical', 'leadership', 'domain', 'soft_skills', 'tools']
    },
    skills: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          skillName: { type: 'string' },
          currentLevel: { type: 'number', min: 1, max: 5 },
          requiredLevel: { type: 'number', min: 1, max: 5 },
          gap: { type: 'number' },
          employeesWithSkill: { type: 'number' },
          employeesNeedingSkill: { type: 'number' },
          criticality: { type: 'string', enum: ['critical', 'important', 'nice_to_have'] }
        }
      }
    },
    gapImpact: {
      type: 'string',
      required: true,
      enum: ['severe', 'moderate', 'minor', 'minimal']
    },
    closureStrategy: {
      type: 'array',
      required: true,
      items: {
        type: 'string',
        enum: ['training', 'hiring', 'contracting', 'partnering', 'outsourcing']
      }
    },
    timeToClose: { type: 'number', required: true }, // months
    investmentRequired: { type: 'number', required: true }, // dollars
    successProbability: { type: 'number', required: true } // percentage
  };

  const result = await synth.generateStructured({
    count: 100,
    schema: skillGapSchema,
    format: 'json',
    context: `Generate skill gap analysis:
    - Critical gaps (20%): Cloud architecture, ML/AI, cybersecurity
    - Important gaps (50%): Modern frameworks, data engineering, API design
    - Nice-to-have gaps (30%): Additional languages, tools, certifications
    - Average gap: 1.2 levels
    - Severe impact: blocks strategic initiatives
    - Training: 6-12 months for technical skills
    - Hiring: 3-6 months to close gaps
    - Contracting: immediate but expensive
    - Success probability: Training 70%, Hiring 85%, Contract 95%
    - Investment: $5K-$15K per person for training
    - Include emerging skills (AI, Web3, etc.)`
  });

  return result;
}

/**
 * Generate turnover prediction data
 * Models attrition risk and retention strategies
 */
export async function generateTurnoverPredictions() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const turnoverSchema = {
    employeeId: { type: 'string', required: true },
    predictionDate: { type: 'string', required: true },
    tenure: { type: 'number', required: true }, // years
    role: { type: 'string', required: true },
    level: { type: 'string', required: true },
    flightRiskScore: {
      type: 'number',
      required: true,
      min: 0,
      max: 100
    },
    riskCategory: {
      type: 'string',
      required: true,
      enum: ['low', 'medium', 'high', 'critical']
    },
    riskFactors: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          factor: { type: 'string' },
          impact: { type: 'number', min: 0, max: 10 }
        }
      }
    },
    retentionActions: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          action: { type: 'string' },
          effectiveness: { type: 'number', min: 0, max: 100 },
          cost: { type: 'number' }
        }
      }
    },
    replacementCost: { type: 'number', required: true },
    businessImpact: {
      type: 'string',
      required: true,
      enum: ['critical', 'high', 'moderate', 'low']
    },
    probabilityOfLeaving: { type: 'number', required: true }, // 0-1
    timeframe: {
      type: 'string',
      required: true,
      enum: ['0-3_months', '3-6_months', '6-12_months', '12+_months']
    }
  };

  const result = await synth.generateStructured({
    count: 300,
    schema: turnoverSchema,
    format: 'json',
    context: `Generate turnover prediction data:
    - Overall risk: Low 60%, Medium 25%, High 12%, Critical 3%
    - Risk factors: Compensation (30%), growth (25%), manager (20%), workload (15%), culture (10%)
    - High risk: tenure 1-2 years or 5-7 years, below-market comp, low engagement
    - Retention actions: compensation adjustment (60% effective), promotion (70%), project change (40%)
    - Replacement cost: 1.5-2x annual salary
    - Critical business impact: key person dependencies, unique skills
    - Probability: High risk 60-80%, Medium 30-50%, Low <20%
    - Time patterns: Q1 and post-review periods highest risk
    - Include false positives and negatives for realism`
  });

  return result;
}

/**
 * Generate compensation analysis data
 * Models pay equity, market positioning, and adjustment needs
 */
export async function generateCompensationAnalysis() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const compensationSchema = {
    analysisId: { type: 'string', required: true },
    analysisDate: { type: 'string', required: true },
    jobFamily: { type: 'string', required: true },
    level: { type: 'string', required: true },
    location: { type: 'string', required: true },
    employeeCount: { type: 'number', required: true },
    salaryData: {
      type: 'object',
      required: true,
      properties: {
        min: { type: 'number' },
        max: { type: 'number' },
        median: { type: 'number' },
        mean: { type: 'number' },
        stddev: { type: 'number' }
      }
    },
    marketData: {
      type: 'object',
      required: true,
      properties: {
        p25: { type: 'number' },
        p50: { type: 'number' },
        p75: { type: 'number' },
        p90: { type: 'number' }
      }
    },
    positioning: {
      type: 'string',
      required: true,
      enum: ['below_market', 'at_market', 'above_market']
    },
    equityAnalysis: {
      type: 'object',
      required: true,
      properties: {
        genderPayGap: { type: 'number' }, // percentage
        minorityPayGap: { type: 'number' },
        tenureDisparity: { type: 'number' },
        equitable: { type: 'boolean' }
      }
    },
    adjustmentNeeds: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          employeeId: { type: 'string' },
          currentSalary: { type: 'number' },
          recommendedSalary: { type: 'number' },
          adjustmentPercent: { type: 'number' },
          reason: { type: 'string' }
        }
      }
    },
    budgetRequired: { type: 'number', required: true },
    complianceStatus: {
      type: 'string',
      required: true,
      enum: ['compliant', 'needs_review', 'non_compliant']
    }
  };

  const result = await synth.generateStructured({
    count: 50,
    schema: compensationSchema,
    format: 'json',
    context: `Generate compensation analysis data:
    - Market positioning: 40% at market, 35% below, 25% above
    - Target: P50-P65 of market for most roles
    - Critical roles: P75-P90
    - Gender pay gap: 2-8% (raw), 0-2% (adjusted for role/tenure)
    - Pay equity: Identify and flag >5% unexplained gaps
    - Adjustment needs: 25% of employees (2-15% increases)
    - Reasons: market adjustment, equity correction, retention risk
    - Budget: 2-4% of total compensation budget
    - Location variance: SF/NYC +30%, Austin +10%, Remote -5%
    - Include both base salary and total compensation
    - Flag compliance issues (pay transparency, equal pay)`
  });

  return result;
}

/**
 * Generate career progression path data
 * Models typical career ladders and advancement timelines
 */
export async function generateCareerPaths() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const careerPathSchema = {
    pathId: { type: 'string', required: true },
    jobFamily: { type: 'string', required: true },
    track: {
      type: 'string',
      required: true,
      enum: ['individual_contributor', 'management', 'technical_leadership']
    },
    levels: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          level: { type: 'string' },
          title: { type: 'string' },
          typicalTenure: { type: 'number' }, // years at level
          keyCompetencies: {
            type: 'array',
            items: { type: 'string' }
          },
          salaryRange: {
            type: 'object',
            properties: {
              min: { type: 'number' },
              max: { type: 'number' }
            }
          }
        }
      }
    },
    totalCareerLength: { type: 'number', required: true }, // years entry to senior
    advancementRate: {
      type: 'object',
      required: true,
      properties: {
        fast: { type: 'number' }, // years
        typical: { type: 'number' },
        slow: { type: 'number' }
      }
    },
    transitionPoints: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          from: { type: 'string' },
          to: { type: 'string' },
          successRate: { type: 'number' }, // percentage
          requiredDevelopment: {
            type: 'array',
            items: { type: 'string' }
          }
        }
      }
    },
    lateralMoves: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          toJobFamily: { type: 'string' },
          feasibility: { type: 'string', enum: ['common', 'possible', 'rare'] },
          skillTransfer: { type: 'number' } // percentage
        }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 30,
    schema: careerPathSchema,
    format: 'json',
    context: `Generate career path data:
    - IC track: Junior (1-2y) → Mid (2-4y) → Senior (3-5y) → Staff (4-6y) → Principal (5+y)
    - Management track: IC → Lead (3-5y) → Manager (2-3y) → Sr Mgr (3-4y) → Director (4-6y)
    - Fast advancement: Top 15%, 50% faster than typical
    - Slow advancement: Bottom 20%, 50% slower than typical
    - IC to management: 40% attempt, 70% succeed
    - Management to IC: 20% return, 90% succeed
    - Lateral moves: Engineering ↔ Product (possible), Sales ↔ Marketing (common)
    - Skill transfer: Same domain 80%, Adjacent 50%, Different 20%
    - Salary growth: 8-12% per promotion, 3-5% annual merit
    - Include diverse paths and non-linear progressions`
  });

  return result;
}

/**
 * Generate workforce diversity metrics
 * Models representation, inclusion, and equity indicators
 */
export async function generateDiversityMetrics() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const diversitySchema = {
    reportingPeriod: { type: 'string', required: true },
    organizationLevel: {
      type: 'string',
      required: true,
      enum: ['company', 'division', 'department', 'team']
    },
    entityName: { type: 'string', required: true },
    headcount: { type: 'number', required: true },
    demographics: {
      type: 'object',
      required: true,
      properties: {
        gender: {
          type: 'object',
          properties: {
            women: { type: 'number' }, // percentage
            men: { type: 'number' },
            nonBinary: { type: 'number' },
            undisclosed: { type: 'number' }
          }
        },
        ethnicity: {
          type: 'object',
          properties: {
            asian: { type: 'number' },
            black: { type: 'number' },
            hispanic: { type: 'number' },
            white: { type: 'number' },
            multiracial: { type: 'number' },
            other: { type: 'number' },
            undisclosed: { type: 'number' }
          }
        },
        age: {
          type: 'object',
          properties: {
            under30: { type: 'number' },
            age30to40: { type: 'number' },
            age40to50: { type: 'number' },
            over50: { type: 'number' }
          }
        }
      }
    },
    representation: {
      type: 'object',
      required: true,
      properties: {
        leadership: {
          type: 'object',
          properties: {
            women: { type: 'number' },
            minorities: { type: 'number' }
          }
        },
        technical: {
          type: 'object',
          properties: {
            women: { type: 'number' },
            minorities: { type: 'number' }
          }
        },
        hiring: {
          type: 'object',
          properties: {
            women: { type: 'number' },
            minorities: { type: 'number' }
          }
        },
        promotions: {
          type: 'object',
          properties: {
            women: { type: 'number' },
            minorities: { type: 'number' }
          }
        }
      }
    },
    inclusionMetrics: {
      type: 'object',
      required: true,
      properties: {
        belongingScore: { type: 'number', min: 0, max: 100 },
        psychologicalSafety: { type: 'number', min: 0, max: 100 },
        fairTreatment: { type: 'number', min: 0, max: 100 },
        voiceHeard: { type: 'number', min: 0, max: 100 }
      }
    },
    trends: {
      type: 'object',
      required: true,
      properties: {
        direction: { type: 'string', enum: ['improving', 'stable', 'declining'] },
        yearOverYearChange: { type: 'number' } // percentage points
      }
    },
    goals: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          metric: { type: 'string' },
          current: { type: 'number' },
          target: { type: 'number' },
          deadline: { type: 'string' }
        }
      }
    }
  };

  const result = await synth.generateStructured({
    count: 60,
    schema: diversitySchema,
    format: 'json',
    context: `Generate diversity metrics (based on tech industry benchmarks):
    - Overall women: 25-35% (improving +1-2% annually)
    - Technical women: 20-30%
    - Leadership women: 25-40%
    - Underrepresented minorities: 15-25%
    - Age distribution: 40% under 30, 35% 30-40, 20% 40-50, 5% over 50
    - Hiring: Should meet or exceed current representation
    - Promotions: Within ±3% of representation (equity indicator)
    - Inclusion scores: 70-85 range, correlates with diversity
    - Belonging: Higher in diverse teams (+10-15 points)
    - Pipeline challenge: Narrowing at senior levels
    - Goals: 40% women in tech by 2025 (aspirational)
    - Include intersectional data where appropriate`
  });

  return result;
}

/**
 * Run all workforce planning examples
 */
export async function runAllWorkforcePlanningExamples() {
  console.log('=== Workforce Planning Simulation Examples ===\n');

  console.log('1. Generating Hiring Forecasts...');
  const hiring = await generateHiringForecast();
  console.log(`Generated ${hiring.data.length} hiring forecast records`);
  console.log('Sample:', JSON.stringify(hiring.data[0], null, 2));

  console.log('\n2. Generating Skill Gap Analysis...');
  const skills = await generateSkillGapAnalysis();
  console.log(`Generated ${skills.data.length} skill gap records`);
  console.log('Sample:', JSON.stringify(skills.data[0], null, 2));

  console.log('\n3. Generating Turnover Predictions...');
  const turnover = await generateTurnoverPredictions();
  console.log(`Generated ${turnover.data.length} turnover prediction records`);
  console.log('Sample:', JSON.stringify(turnover.data[0], null, 2));

  console.log('\n4. Generating Compensation Analysis...');
  const compensation = await generateCompensationAnalysis();
  console.log(`Generated ${compensation.data.length} compensation analysis records`);
  console.log('Sample:', JSON.stringify(compensation.data[0], null, 2));

  console.log('\n5. Generating Career Paths...');
  const careers = await generateCareerPaths();
  console.log(`Generated ${careers.data.length} career path records`);
  console.log('Sample:', JSON.stringify(careers.data[0], null, 2));

  console.log('\n6. Generating Diversity Metrics...');
  const diversity = await generateDiversityMetrics();
  console.log(`Generated ${diversity.data.length} diversity metric records`);
  console.log('Sample:', JSON.stringify(diversity.data[0], null, 2));
}

// Uncomment to run
// runAllWorkforcePlanningExamples().catch(console.error);
