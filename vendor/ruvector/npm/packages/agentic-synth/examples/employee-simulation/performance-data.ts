/**
 * Employee Performance Data Simulation
 *
 * Generates realistic KPI achievement data, project deliverables, code metrics,
 * sales targets, quality metrics, and learning progress for performance analysis.
 *
 * ETHICS NOTE: Performance simulations should be used for system testing only.
 * Never use synthetic data to make actual decisions about real employees.
 */

import { createSynth } from '../../src/index.js';

/**
 * Generate KPI achievement data
 * Models diverse performance metrics across different roles
 */
export async function generateKPIData() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const kpiSchema = {
    employeeId: { type: 'string', required: true },
    quarter: { type: 'string', required: true },
    role: {
      type: 'string',
      required: true,
      enum: ['engineer', 'designer', 'product_manager', 'sales', 'marketing', 'support']
    },
    kpis: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          name: { type: 'string' },
          target: { type: 'number' },
          actual: { type: 'number' },
          unit: { type: 'string' },
          weight: { type: 'number' } // percentage of total performance
        }
      }
    },
    overallScore: {
      type: 'number',
      required: true,
      min: 0,
      max: 100
    },
    rating: {
      type: 'string',
      required: true,
      enum: ['exceeds', 'meets', 'developing', 'needs_improvement']
    },
    notes: { type: 'string', required: false }
  };

  const result = await synth.generateStructured({
    count: 400,
    schema: kpiSchema,
    format: 'json',
    context: `Generate realistic KPI data with normal distribution:
    - Exceeds expectations: 15%
    - Meets expectations: 70%
    - Developing: 10%
    - Needs improvement: 5%
    - Engineer KPIs: story points, code quality, bug fix rate
    - Sales KPIs: revenue, deals closed, pipeline value
    - Support KPIs: CSAT, resolution time, ticket volume
    - Include realistic variance and outliers
    - Consider external factors (market conditions, resources)`
  });

  return result;
}

/**
 * Generate project deliverables tracking
 * Models project contributions and completion quality
 */
export async function generateProjectDeliverables() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const deliverableSchema = {
    projectId: { type: 'string', required: true },
    employeeId: { type: 'string', required: true },
    deliverable: { type: 'string', required: true },
    deliveryDate: { type: 'string', required: true },
    dueDate: { type: 'string', required: true },
    completionStatus: {
      type: 'string',
      required: true,
      enum: ['on_time', 'early', 'late', 'partial']
    },
    qualityRating: {
      type: 'number',
      required: true,
      min: 1,
      max: 5
    },
    scope: {
      type: 'string',
      required: true,
      enum: ['as_planned', 'expanded', 'reduced']
    },
    stakeholderSatisfaction: {
      type: 'number',
      required: true,
      min: 1,
      max: 10
    },
    technicalDebt: {
      type: 'string',
      required: true,
      enum: ['none', 'minimal', 'moderate', 'significant']
    },
    reworkRequired: { type: 'boolean', required: true }
  };

  const result = await synth.generateStructured({
    count: 600,
    schema: deliverableSchema,
    format: 'json',
    context: `Generate realistic project deliverables:
    - 65% on-time delivery
    - 15% early delivery
    - 20% late delivery (avg 3-5 days)
    - Quality rating: normal distribution around 4.0
    - 10% require rework
    - Scope changes: 60% as planned, 25% expanded, 15% reduced
    - Stakeholder satisfaction correlates with on-time + quality
    - Technical debt: 50% minimal, 30% moderate, 15% significant, 5% none`
  });

  return result;
}

/**
 * Generate code commits and review metrics (for developers)
 * Models realistic development activity and code quality
 */
export async function generateCodeMetrics() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const codeMetricsSchema = {
    employeeId: { type: 'string', required: true },
    week: { type: 'string', required: true },
    commits: {
      type: 'object',
      required: true,
      properties: {
        count: { type: 'number' },
        linesAdded: { type: 'number' },
        linesRemoved: { type: 'number' },
        filesChanged: { type: 'number' }
      }
    },
    pullRequests: {
      type: 'object',
      required: true,
      properties: {
        opened: { type: 'number' },
        merged: { type: 'number' },
        avgReviewTime: { type: 'number' }, // hours
        avgSize: { type: 'number' } // lines changed
      }
    },
    codeReviews: {
      type: 'object',
      required: true,
      properties: {
        reviewsGiven: { type: 'number' },
        commentsPosted: { type: 'number' },
        avgResponseTime: { type: 'number' } // hours
      }
    },
    quality: {
      type: 'object',
      required: true,
      properties: {
        testCoverage: { type: 'number' }, // percentage
        bugsFound: { type: 'number' },
        codeSmells: { type: 'number' },
        securityIssues: { type: 'number' }
      }
    },
    productivity: {
      type: 'object',
      required: true,
      properties: {
        storyPointsCompleted: { type: 'number' },
        velocity: { type: 'number' }
      }
    }
  };

  const result = await synth.generateTimeSeries({
    count: 500,
    interval: '1w',
    metrics: ['commits.count', 'quality.testCoverage', 'productivity.velocity'],
    trend: 'stable',
    seasonality: false,
    context: `Generate realistic code metrics for diverse developers:
    - Senior devs: 15-25 commits/week, 80-90% test coverage
    - Mid-level: 10-20 commits/week, 70-80% test coverage
    - Junior: 5-15 commits/week, 60-75% test coverage
    - PR size: 100-500 lines optimal
    - Review time: 2-24 hours (median 6h)
    - Bugs found: inverse correlation with experience
    - Include realistic variations: vacation weeks, sprint cycles
    - Quality-focused devs: fewer commits, higher coverage`
  });

  return result;
}

/**
 * Generate sales targets and achievements
 * Models sales performance with realistic quota attainment
 */
export async function generateSalesPerformance() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const salesSchema = {
    employeeId: { type: 'string', required: true },
    month: { type: 'string', required: true },
    territory: { type: 'string', required: true },
    quota: { type: 'number', required: true },
    revenue: { type: 'number', required: true },
    quotaAttainment: { type: 'number', required: true }, // percentage
    dealsWon: { type: 'number', required: true },
    dealsLost: { type: 'number', required: true },
    winRate: { type: 'number', required: true }, // percentage
    avgDealSize: { type: 'number', required: true },
    pipelineValue: { type: 'number', required: true },
    newLeads: { type: 'number', required: true },
    customerRetention: { type: 'number', required: true }, // percentage
    upsellRevenue: { type: 'number', required: true }
  };

  const result = await synth.generateStructured({
    count: 300,
    schema: salesSchema,
    format: 'json',
    context: `Generate realistic sales performance data:
    - Quota attainment: normal distribution, mean 85%, stddev 20%
    - 40% hit or exceed quota
    - Win rate: 20-40% range
    - Seasonal patterns: Q4 spike, Q1 dip
    - Territory impact: ±15% variance
    - Top performers: 120%+ attainment (15% of team)
    - Struggling reps: <60% attainment (10% of team)
    - Pipeline: 3-5x quota
    - Include slump periods and recovery`
  });

  return result;
}

/**
 * Generate quality metrics across different roles
 * Models output quality and accuracy
 */
export async function generateQualityMetrics() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const qualitySchema = {
    employeeId: { type: 'string', required: true },
    week: { type: 'string', required: true },
    role: { type: 'string', required: true },
    outputs: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          outputType: { type: 'string' },
          count: { type: 'number' },
          defectRate: { type: 'number' }, // percentage
          reworkRate: { type: 'number' }, // percentage
          peerRating: { type: 'number' } // 1-5
        }
      }
    },
    customerFeedback: {
      type: 'object',
      required: true,
      properties: {
        nps: { type: 'number' }, // -100 to 100
        csat: { type: 'number' }, // 1-5
        feedbackCount: { type: 'number' }
      }
    },
    consistencyScore: {
      type: 'number',
      required: true,
      min: 0,
      max: 100
    },
    innovationScore: {
      type: 'number',
      required: true,
      min: 0,
      max: 100
    }
  };

  const result = await synth.generateStructured({
    count: 400,
    schema: qualitySchema,
    format: 'json',
    context: `Generate quality metrics by role:
    - Engineers: defect rate 1-5%, code review score 3.5-4.5
    - Designers: peer rating 3.8-4.8, iteration count 2-5
    - Support: CSAT 4.2-4.8, first contact resolution 70-85%
    - Content: error rate 0.5-2%, engagement metrics
    - Quality improves with experience (10-15% difference)
    - Consistency vs Innovation tradeoff
    - Include learning curves and improvement trends`
  });

  return result;
}

/**
 * Generate learning and development progress
 * Models continuous learning and skill acquisition
 */
export async function generateLearningProgress() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const learningSchema = {
    employeeId: { type: 'string', required: true },
    quarter: { type: 'string', required: true },
    coursesCompleted: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          courseName: { type: 'string' },
          category: { type: 'string' },
          hoursSpent: { type: 'number' },
          completionRate: { type: 'number' }, // percentage
          assessmentScore: { type: 'number' }, // percentage
          certification: { type: 'boolean' }
        }
      }
    },
    skillsAcquired: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          skill: { type: 'string' },
          level: { type: 'string', enum: ['beginner', 'intermediate', 'advanced'] },
          verifiedBy: { type: 'string' }
        }
      }
    },
    mentoring: {
      type: 'object',
      required: true,
      properties: {
        hoursReceived: { type: 'number' },
        hoursGiven: { type: 'number' },
        mentees: { type: 'number' }
      }
    },
    learningHoursGoal: { type: 'number', required: true },
    learningHoursActual: { type: 'number', required: true },
    learningBudgetUsed: { type: 'number', required: true }, // dollars
    applicationScore: {
      type: 'number',
      required: true,
      min: 0,
      max: 10
    }
  };

  const result = await synth.generateStructured({
    count: 350,
    schema: learningSchema,
    format: 'json',
    context: `Generate learning and development data:
    - Goal: 40 hours per quarter per employee
    - Actual: normal distribution, mean 35 hours, stddev 12
    - 70% achieve 80%+ of learning goal
    - Assessment scores: mean 82%, stddev 10%
    - Course completion: 85% average
    - Senior employees: more mentoring given
    - Junior employees: more mentoring received
    - Application score: practical use of learning
    - Budget: $1000-$3000 per employee per year
    - Include self-directed vs formal training mix`
  });

  return result;
}

/**
 * Generate comprehensive performance review data
 * Models 360-degree feedback and competency ratings
 */
export async function generatePerformanceReviews() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const reviewSchema = {
    employeeId: { type: 'string', required: true },
    reviewPeriod: { type: 'string', required: true },
    reviewType: {
      type: 'string',
      required: true,
      enum: ['annual', 'mid_year', '90_day', 'probation']
    },
    competencies: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          name: { type: 'string' },
          rating: { type: 'number', min: 1, max: 5 },
          selfRating: { type: 'number', min: 1, max: 5 },
          managerRating: { type: 'number', min: 1, max: 5 },
          peerRating: { type: 'number', min: 1, max: 5 }
        }
      }
    },
    strengths: {
      type: 'array',
      required: true,
      items: { type: 'string' }
    },
    areasForImprovement: {
      type: 'array',
      required: true,
      items: { type: 'string' }
    },
    goals: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          goal: { type: 'string' },
          achieved: { type: 'number' }, // percentage
          impact: { type: 'string' }
        }
      }
    },
    overallRating: {
      type: 'string',
      required: true,
      enum: ['outstanding', 'exceeds', 'meets', 'developing', 'unsatisfactory']
    },
    promotionReady: { type: 'boolean', required: true },
    riskOfAttrition: {
      type: 'string',
      required: true,
      enum: ['low', 'medium', 'high']
    }
  };

  const result = await synth.generateStructured({
    count: 250,
    schema: reviewSchema,
    format: 'json',
    context: `Generate performance review data with realistic distributions:
    - Overall ratings: Outstanding 10%, Exceeds 20%, Meets 60%, Developing 8%, Unsatisfactory 2%
    - Self-ratings typically 0.3-0.5 points higher than manager
    - Peer ratings most accurate (closest to actual performance)
    - 3-5 strengths, 2-3 areas for improvement
    - 3-5 goals per review period
    - 70% of goals fully or mostly achieved
    - Promotion ready: 15% of workforce
    - Attrition risk: Low 70%, Medium 20%, High 10%
    - Include diversity in feedback styles and competencies`
  });

  return result;
}

/**
 * Run all performance data examples
 */
export async function runAllPerformanceExamples() {
  console.log('=== Employee Performance Data Simulation Examples ===\n');

  console.log('1. Generating KPI Data...');
  const kpis = await generateKPIData();
  console.log(`Generated ${kpis.data.length} KPI records`);
  console.log('Sample:', JSON.stringify(kpis.data[0], null, 2));

  console.log('\n2. Generating Project Deliverables...');
  const deliverables = await generateProjectDeliverables();
  console.log(`Generated ${deliverables.data.length} deliverable records`);
  console.log('Sample:', JSON.stringify(deliverables.data[0], null, 2));

  console.log('\n3. Generating Code Metrics...');
  const code = await generateCodeMetrics();
  console.log(`Generated ${code.data.length} code metric records`);
  console.log('Sample:', JSON.stringify(code.data[0], null, 2));

  console.log('\n4. Generating Sales Performance...');
  const sales = await generateSalesPerformance();
  console.log(`Generated ${sales.data.length} sales records`);
  console.log('Sample:', JSON.stringify(sales.data[0], null, 2));

  console.log('\n5. Generating Quality Metrics...');
  const quality = await generateQualityMetrics();
  console.log(`Generated ${quality.data.length} quality metric records`);
  console.log('Sample:', JSON.stringify(quality.data[0], null, 2));

  console.log('\n6. Generating Learning Progress...');
  const learning = await generateLearningProgress();
  console.log(`Generated ${learning.data.length} learning records`);
  console.log('Sample:', JSON.stringify(learning.data[0], null, 2));

  console.log('\n7. Generating Performance Reviews...');
  const reviews = await generatePerformanceReviews();
  console.log(`Generated ${reviews.data.length} review records`);
  console.log('Sample:', JSON.stringify(reviews.data[0], null, 2));
}

// Uncomment to run
// runAllPerformanceExamples().catch(console.error);
