/**
 * Workplace Events Simulation
 *
 * Generates realistic workplace events including onboarding, offboarding, promotions,
 * performance reviews, training, team building, and conflict resolution scenarios.
 *
 * RESPONSIBLE USE: These simulations are for HR system testing and process optimization.
 * Handle sensitive event data with appropriate privacy and security measures.
 */

import { createSynth } from '../../src/index.js';

/**
 * Generate employee onboarding events
 * Models the new hire journey and integration process
 */
export async function generateOnboardingEvents() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const onboardingSchema = {
    eventId: { type: 'string', required: true },
    employeeId: { type: 'string', required: true },
    startDate: { type: 'string', required: true },
    role: { type: 'string', required: true },
    department: { type: 'string', required: true },
    workMode: {
      type: 'string',
      required: true,
      enum: ['onsite', 'remote', 'hybrid']
    },
    onboardingPlan: {
      type: 'object',
      required: true,
      properties: {
        duration: { type: 'number' }, // days
        checkpoints: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              day: { type: 'number' },
              milestone: { type: 'string' },
              completed: { type: 'boolean' }
            }
          }
        }
      }
    },
    assignments: {
      type: 'object',
      required: true,
      properties: {
        buddy: { type: 'string' },
        mentor: { type: 'string' },
        manager: { type: 'string' },
        team: { type: 'string' }
      }
    },
    progress: {
      type: 'object',
      required: true,
      properties: {
        completionRate: { type: 'number' }, // percentage
        firstWeekEngagement: { type: 'number' }, // 1-10
        firstMonthPerformance: { type: 'number' }, // 1-10
        buddyMeetings: { type: 'number' },
        trainingCompleted: { type: 'number' }
      }
    },
    feedback: {
      type: 'object',
      required: true,
      properties: {
        onboardingExperience: { type: 'number', min: 1, max: 5 },
        clarityOfExpectations: { type: 'number', min: 1, max: 5 },
        toolsReadiness: { type: 'number', min: 1, max: 5 },
        teamWelcome: { type: 'number', min: 1, max: 5 }
      }
    },
    outcomes: {
      type: 'object',
      required: true,
      properties: {
        timeToProductivity: { type: 'number' }, // days
        retainedAfter90Days: { type: 'boolean' },
        retainedAfter1Year: { type: 'boolean' }
      }
    }
  };

  const result = await synth.generateEvents({
    count: 200,
    eventTypes: ['onboarding_start', 'day_1', 'week_1', 'month_1', 'day_90'],
    distribution: 'normal',
    timeRange: {
      start: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000), // 1 year
      end: new Date()
    },
    context: `Generate onboarding events:
    - Onboarding duration: 30-90 days (60 typical)
    - Completion rate: 85% complete all milestones
    - Time to productivity: Junior 60-90 days, Senior 30-45 days
    - First week critical: High engagement predicts retention (+30%)
    - Buddy program: 4-8 meetings in first month
    - Remote onboarding: Requires more check-ins (+50%)
    - 90-day retention: 92%
    - 1-year retention: 78%
    - Satisfaction: mean 4.2/5
    - Common issues: Tools setup (20%), unclear expectations (15%)
    - Include both smooth and challenging onboardings`
  });

  return result;
}

/**
 * Generate employee offboarding events
 * Models departure process and exit analytics
 */
export async function generateOffboardingEvents() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const offboardingSchema = {
    eventId: { type: 'string', required: true },
    employeeId: { type: 'string', required: true },
    lastDay: { type: 'string', required: true },
    tenure: { type: 'number', required: true }, // years
    role: { type: 'string', required: true },
    department: { type: 'string', required: true },
    separationType: {
      type: 'string',
      required: true,
      enum: ['voluntary', 'involuntary', 'retirement', 'contract_end', 'relocation']
    },
    reason: {
      type: 'string',
      required: true,
      enum: ['better_opportunity', 'compensation', 'career_growth', 'management', 'culture', 'work_life_balance', 'performance', 'restructuring', 'personal']
    },
    exitInterview: {
      type: 'object',
      required: true,
      properties: {
        completed: { type: 'boolean' },
        overallSatisfaction: { type: 'number', min: 1, max: 5 },
        wouldRecommend: { type: 'boolean' },
        wouldReturn: { type: 'boolean' },
        managerRating: { type: 'number', min: 1, max: 5 },
        topPositive: { type: 'string' },
        topImprovement: { type: 'string' }
      }
    },
    notice: {
      type: 'object',
      required: true,
      properties: {
        givenDays: { type: 'number' },
        standardDays: { type: 'number' },
        counteroffer: { type: 'boolean' },
        counterofferAccepted: { type: 'boolean' }
      }
    },
    transition: {
      type: 'object',
      required: true,
      properties: {
        knowledgeTransfer: { type: 'number', min: 0, max: 100 }, // percentage
        documentationComplete: { type: 'boolean' },
        backfillIdentified: { type: 'boolean' },
        teamImpact: { type: 'string', enum: ['minimal', 'moderate', 'significant', 'severe'] }
      }
    },
    rehireEligibility: {
      type: 'string',
      required: true,
      enum: ['yes', 'no', 'conditional']
    }
  };

  const result = await synth.generateEvents({
    count: 150,
    eventTypes: ['resignation', 'termination', 'retirement', 'last_day'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    context: `Generate offboarding events:
    - Voluntary: 75%, Involuntary: 20%, Retirement: 3%, Other: 2%
    - Top reasons: Better opportunity (35%), compensation (25%), growth (20%), management (10%)
    - Notice period: 2 weeks standard, 4 weeks for senior
    - Counteroffer: Made for 30%, accepted 40% of those
    - Exit interview completion: 65%
    - Would recommend: 60% (high correlation with reason)
    - Would return: 45% (boomerang candidates)
    - Knowledge transfer: 70% complete average
    - Tenure patterns: Peak at 1-2 years and 5-7 years
    - Q1 spike: 25% higher than average
    - Rehire eligible: 85% of voluntary departures`
  });

  return result;
}

/**
 * Generate promotion and transfer events
 * Models career advancement and internal mobility
 */
export async function generatePromotionEvents() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const promotionSchema = {
    eventId: { type: 'string', required: true },
    employeeId: { type: 'string', required: true },
    effectiveDate: { type: 'string', required: true },
    eventType: {
      type: 'string',
      required: true,
      enum: ['promotion', 'lateral_move', 'department_transfer', 'location_transfer']
    },
    from: {
      type: 'object',
      required: true,
      properties: {
        title: { type: 'string' },
        level: { type: 'string' },
        department: { type: 'string' },
        salary: { type: 'number' }
      }
    },
    to: {
      type: 'object',
      required: true,
      properties: {
        title: { type: 'string' },
        level: { type: 'string' },
        department: { type: 'string' },
        salary: { type: 'number' }
      }
    },
    tenureBeforeChange: { type: 'number', required: true }, // years
    justification: {
      type: 'array',
      required: true,
      items: { type: 'string' }
    },
    salaryChange: {
      type: 'object',
      required: true,
      properties: {
        amount: { type: 'number' },
        percentage: { type: 'number' }
      }
    },
    competitionConsidered: { type: 'number', required: true },
    readinessAssessment: {
      type: 'object',
      required: true,
      properties: {
        score: { type: 'number', min: 0, max: 100 },
        gapsClosed: { type: 'boolean' },
        supportRequired: { type: 'boolean' }
      }
    },
    outcomes: {
      type: 'object',
      required: true,
      properties: {
        successful: { type: 'boolean' },
        performanceAfter3Months: { type: 'number', min: 1, max: 5 },
        performanceAfter6Months: { type: 'number', min: 1, max: 5 }
      }
    }
  };

  const result = await synth.generateEvents({
    count: 180,
    eventTypes: ['promotion', 'lateral_move', 'transfer'],
    distribution: 'normal',
    timeRange: {
      start: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    context: `Generate promotion/transfer events:
    - Promotions: 60%, Lateral: 25%, Transfers: 15%
    - Promotion rate: 15% of workforce annually
    - Tenure before promotion: 2-4 years average
    - Salary increase: Promotion 10-20%, Lateral 0-5%, Transfer varies
    - Readiness: 80% meet criteria, 15% stretch assignments
    - Competition: 2-5 candidates per role
    - Success rate: 85% meet expectations in new role
    - Performance dip in first 3 months (learning curve)
    - Recovery by 6 months (90% at or above baseline)
    - Year-end cycle: 70% of promotions
    - Include diversity in advancement opportunities`
  });

  return result;
}

/**
 * Generate performance review events
 * Models the review cycle and feedback process
 */
export async function generatePerformanceReviewEvents() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const reviewSchema = {
    eventId: { type: 'string', required: true },
    employeeId: { type: 'string', required: true },
    reviewDate: { type: 'string', required: true },
    reviewType: {
      type: 'string',
      required: true,
      enum: ['annual', 'mid_year', 'quarterly', 'probation', 'pip']
    },
    reviewPeriod: { type: 'string', required: true },
    process: {
      type: 'object',
      required: true,
      properties: {
        selfReviewComplete: { type: 'boolean' },
        peerReviewsComplete: { type: 'number' }, // count
        managerReviewComplete: { type: 'boolean' },
        calibrationDone: { type: 'boolean' },
        employeeMeetingDone: { type: 'boolean' }
      }
    },
    ratings: {
      type: 'object',
      required: true,
      properties: {
        overall: { type: 'string', enum: ['exceeds', 'meets', 'developing', 'unsatisfactory'] },
        selfRating: { type: 'string' },
        managerRating: { type: 'string' },
        calibratedRating: { type: 'string' }
      }
    },
    outcomes: {
      type: 'object',
      required: true,
      properties: {
        meritIncrease: { type: 'number' }, // percentage
        bonus: { type: 'number' }, // dollars
        equity: { type: 'number' }, // shares/units
        promotionRecommended: { type: 'boolean' },
        developmentPlan: { type: 'boolean' }
      }
    },
    goalsForNextPeriod: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          goal: { type: 'string' },
          measureable: { type: 'boolean' },
          timeline: { type: 'string' }
        }
      }
    },
    employeeSatisfaction: {
      type: 'object',
      required: true,
      properties: {
        processFairness: { type: 'number', min: 1, max: 5 },
        ratingAgreement: { type: 'boolean' },
        feedbackQuality: { type: 'number', min: 1, max: 5 },
        goalClarity: { type: 'number', min: 1, max: 5 }
      }
    }
  };

  const result = await synth.generateEvents({
    count: 400,
    eventTypes: ['annual_review', 'mid_year_review', 'quarterly_check_in'],
    distribution: 'normal',
    timeRange: {
      start: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    context: `Generate performance review events:
    - Annual reviews: All employees once per year
    - Mid-year: 70% of organizations
    - Quarterly: 40% of organizations (check-ins)
    - Completion: Self 95%, Manager 98%, Peers 85%
    - Rating distribution: Exceeds 15%, Meets 70%, Developing 12%, Unsatisfactory 3%
    - Self vs manager: 60% match, 30% self higher, 10% manager higher
    - Calibration adjusts 25% of initial ratings
    - Merit increase: Exceeds 4-6%, Meets 2-4%, Developing 0-2%
    - Employee satisfaction: Mean 3.8/5 for process
    - Agreement with rating: 75%
    - Include review anxiety and bias patterns`
  });

  return result;
}

/**
 * Generate training and development events
 * Models learning activities and skill-building programs
 */
export async function generateTrainingEvents() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const trainingSchema = {
    eventId: { type: 'string', required: true },
    eventName: { type: 'string', required: true },
    eventType: {
      type: 'string',
      required: true,
      enum: ['workshop', 'course', 'conference', 'certification', 'lunch_learn', 'hackathon', 'bootcamp']
    },
    date: { type: 'string', required: true },
    duration: { type: 'number', required: true }, // hours
    delivery: {
      type: 'string',
      required: true,
      enum: ['in_person', 'virtual', 'hybrid', 'self_paced']
    },
    topic: {
      type: 'string',
      required: true,
      enum: ['technical', 'leadership', 'soft_skills', 'compliance', 'product', 'industry', 'wellness']
    },
    participants: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          employeeId: { type: 'string' },
          registered: { type: 'boolean' },
          attended: { type: 'boolean' },
          completed: { type: 'boolean' },
          assessmentScore: { type: 'number' }
        }
      }
    },
    metrics: {
      type: 'object',
      required: true,
      properties: {
        capacity: { type: 'number' },
        registered: { type: 'number' },
        attended: { type: 'number' },
        completed: { type: 'number' },
        attendanceRate: { type: 'number' },
        completionRate: { type: 'number' }
      }
    },
    feedback: {
      type: 'object',
      required: true,
      properties: {
        relevance: { type: 'number', min: 1, max: 5 },
        quality: { type: 'number', min: 1, max: 5 },
        applicability: { type: 'number', min: 1, max: 5 },
        wouldRecommend: { type: 'number' } // percentage
      }
    },
    cost: {
      type: 'object',
      required: true,
      properties: {
        perParticipant: { type: 'number' },
        total: { type: 'number' }
      }
    },
    outcomes: {
      type: 'object',
      required: true,
      properties: {
        skillsGained: {
          type: 'array',
          items: { type: 'string' }
        },
        applied: { type: 'number' }, // percentage who applied learning
        impactRating: { type: 'number', min: 1, max: 5 }
      }
    }
  };

  const result = await synth.generateEvents({
    count: 250,
    eventTypes: ['workshop', 'course', 'conference', 'certification', 'lunch_learn'],
    distribution: 'normal',
    timeRange: {
      start: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    context: `Generate training events:
    - Frequency: 2-4 events per month
    - Duration: Workshops 2-4h, Courses 8-40h, Conferences 16-24h
    - Attendance rate: 85% (virtual), 75% (in-person)
    - Completion rate: 70% overall
    - Technical training: Highest demand, 4.3/5 rating
    - Compliance: Mandatory, 95% completion, 3.8/5 rating
    - Leadership: 4.5/5 rating, high impact
    - Lunch & learns: 30-60 mins, 65% attendance
    - Application rate: 55% apply learning within 3 months
    - Cost: $50-$500 per person (external), $0-$100 (internal)
    - Include seasonal patterns (Q1 high, Q4 low)`
  });

  return result;
}

/**
 * Generate team building activity events
 * Models team cohesion and morale activities
 */
export async function generateTeamBuildingEvents() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const teamBuildingSchema = {
    eventId: { type: 'string', required: true },
    eventName: { type: 'string', required: true },
    activityType: {
      type: 'string',
      required: true,
      enum: ['social', 'volunteer', 'offsite', 'workshop', 'competition', 'celebration', 'retreat']
    },
    date: { type: 'string', required: true },
    duration: { type: 'number', required: true }, // hours
    teamId: { type: 'string', required: true },
    scope: {
      type: 'string',
      required: true,
      enum: ['team', 'department', 'division', 'company']
    },
    participation: {
      type: 'object',
      required: true,
      properties: {
        invited: { type: 'number' },
        attended: { type: 'number' },
        rate: { type: 'number' } // percentage
      }
    },
    objectives: {
      type: 'array',
      required: true,
      items: {
        type: 'string',
        enum: ['bonding', 'trust', 'communication', 'collaboration', 'morale', 'recognition', 'fun']
      }
    },
    outcomes: {
      type: 'object',
      required: true,
      properties: {
        enjoyment: { type: 'number', min: 1, max: 5 },
        worthwhile: { type: 'number', min: 1, max: 5 },
        teamCohesion: { type: 'number' }, // change score
        moraleImpact: { type: 'string', enum: ['positive', 'neutral', 'negative'] }
      }
    },
    cost: {
      type: 'object',
      required: true,
      properties: {
        budget: { type: 'number' },
        perPerson: { type: 'number' }
      }
    },
    timing: {
      type: 'object',
      required: true,
      properties: {
        duringWorkHours: { type: 'boolean' },
        optional: { type: 'boolean' }
      }
    }
  };

  const result = await synth.generateEvents({
    count: 100,
    eventTypes: ['social', 'volunteer', 'offsite', 'celebration'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    context: `Generate team building events:
    - Frequency: 1 per quarter per team (minimum)
    - Popular: Happy hours, volunteer days, offsites, game nights
    - Participation: During work hours 85%, After hours 55%
    - Enjoyment: Mean 4.0/5
    - Worthwhile: Mean 3.8/5 (lower for mandatory fun)
    - Budget: $25-$75 per person for social, $500-$2000 for offsites
    - Remote teams: Virtual events 40% participation
    - Morale impact: 80% positive, 15% neutral, 5% negative
    - Best timing: Mid-quarter, avoid month-end
    - Include variety: Some prefer low-key, others adventure`
  });

  return result;
}

/**
 * Generate conflict resolution events
 * Models workplace conflicts and resolution processes
 */
export async function generateConflictResolutionEvents() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const conflictSchema = {
    eventId: { type: 'string', required: true },
    reportDate: { type: 'string', required: true },
    conflictType: {
      type: 'string',
      required: true,
      enum: ['interpersonal', 'performance', 'harassment', 'discrimination', 'policy', 'resource', 'communication']
    },
    severity: {
      type: 'string',
      required: true,
      enum: ['low', 'medium', 'high', 'critical']
    },
    partiesInvolved: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          employeeId: { type: 'string' },
          role: { type: 'string', enum: ['reporter', 'respondent', 'witness', 'affected'] }
        }
      }
    },
    reportedBy: {
      type: 'string',
      required: true,
      enum: ['employee', 'manager', 'peer', 'hr', 'anonymous']
    },
    investigation: {
      type: 'object',
      required: true,
      properties: {
        required: { type: 'boolean' },
        duration: { type: 'number' }, // days
        interviewsConducted: { type: 'number' },
        finding: { type: 'string', enum: ['substantiated', 'unsubstantiated', 'partially_substantiated', 'inconclusive'] }
      }
    },
    resolution: {
      type: 'object',
      required: true,
      properties: {
        approach: {
          type: 'string',
          enum: ['mediation', 'coaching', 'training', 'policy_clarification', 'disciplinary', 'separation', 'team_restructure']
        },
        timeToResolve: { type: 'number' }, // days
        outcome: { type: 'string', enum: ['resolved', 'improved', 'ongoing', 'escalated'] }
      }
    },
    followUp: {
      type: 'object',
      required: true,
      properties: {
        required: { type: 'boolean' },
        checkIns: { type: 'number' },
        recurrence: { type: 'boolean' }
      }
    },
    impact: {
      type: 'object',
      required: true,
      properties: {
        teamMoraleAffected: { type: 'boolean' },
        productivityImpact: { type: 'number' }, // percentage
        turnoverResult: { type: 'boolean' }
      }
    }
  };

  const result = await synth.generateEvents({
    count: 80,
    eventTypes: ['conflict_report', 'investigation', 'mediation', 'resolution'],
    distribution: 'poisson',
    timeRange: {
      start: new Date(Date.now() - 365 * 24 * 60 * 60 * 1000),
      end: new Date()
    },
    context: `Generate conflict resolution events:
    - Rate: 5-10% of workforce involved annually
    - Types: Interpersonal 40%, Performance 25%, Communication 20%, Policy 10%, Others 5%
    - Severity: Low 50%, Medium 35%, High 12%, Critical 3%
    - Investigation: Required for 30% of cases
    - Time to resolve: Low 5-10 days, Medium 10-20 days, High 20-60 days
    - Approaches: Mediation 40%, Coaching 30%, Training 15%, Other 15%
    - Outcomes: Resolved 60%, Improved 25%, Ongoing 10%, Escalated 5%
    - Recurrence: 15% have follow-up issues
    - Turnover: 20% result in departure within 6 months
    - Early intervention: 75% success rate
    - Include sensitive handling and confidentiality`
  });

  return result;
}

/**
 * Run all workplace event examples
 */
export async function runAllWorkplaceEventExamples() {
  console.log('=== Workplace Events Simulation Examples ===\n');

  console.log('1. Generating Onboarding Events...');
  const onboarding = await generateOnboardingEvents();
  console.log(`Generated ${onboarding.data.length} onboarding event records`);
  console.log('Sample:', JSON.stringify(onboarding.data[0], null, 2));

  console.log('\n2. Generating Offboarding Events...');
  const offboarding = await generateOffboardingEvents();
  console.log(`Generated ${offboarding.data.length} offboarding event records`);
  console.log('Sample:', JSON.stringify(offboarding.data[0], null, 2));

  console.log('\n3. Generating Promotion Events...');
  const promotions = await generatePromotionEvents();
  console.log(`Generated ${promotions.data.length} promotion event records`);
  console.log('Sample:', JSON.stringify(promotions.data[0], null, 2));

  console.log('\n4. Generating Performance Review Events...');
  const reviews = await generatePerformanceReviewEvents();
  console.log(`Generated ${reviews.data.length} review event records`);
  console.log('Sample:', JSON.stringify(reviews.data[0], null, 2));

  console.log('\n5. Generating Training Events...');
  const training = await generateTrainingEvents();
  console.log(`Generated ${training.data.length} training event records`);
  console.log('Sample:', JSON.stringify(training.data[0], null, 2));

  console.log('\n6. Generating Team Building Events...');
  const teamBuilding = await generateTeamBuildingEvents();
  console.log(`Generated ${teamBuilding.data.length} team building event records`);
  console.log('Sample:', JSON.stringify(teamBuilding.data[0], null, 2));

  console.log('\n7. Generating Conflict Resolution Events...');
  const conflicts = await generateConflictResolutionEvents();
  console.log(`Generated ${conflicts.data.length} conflict resolution event records`);
  console.log('Sample:', JSON.stringify(conflicts.data[0], null, 2));
}

// Uncomment to run
// runAllWorkplaceEventExamples().catch(console.error);
