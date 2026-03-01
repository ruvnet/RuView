/**
 * Employee Behavior Patterns Simulation
 *
 * Generates realistic daily work schedules, productivity patterns, collaboration,
 * and communication behaviors for workforce modeling.
 *
 * PRIVACY NOTE: All data is synthetic. No real employee data is used or should
 * be used to train these models without explicit consent and proper anonymization.
 */

import { createSynth } from '../../src/index.js';

/**
 * Generate daily work schedule patterns
 * Simulates diverse work hours including flexible schedules, remote work, etc.
 */
export async function generateWorkSchedules() {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  const scheduleSchema = {
    employeeId: { type: 'string', required: true },
    date: { type: 'string', required: true },
    workMode: {
      type: 'string',
      required: true,
      enum: ['office', 'remote', 'hybrid']
    },
    checkIn: { type: 'string', required: true }, // ISO time
    checkOut: { type: 'string', required: true },
    breaks: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          start: { type: 'string' },
          duration: { type: 'number' } // minutes
        }
      }
    },
    overtimeMinutes: { type: 'number', required: false },
    timezone: { type: 'string', required: true }
  };

  const result = await synth.generateStructured({
    count: 500,
    schema: scheduleSchema,
    format: 'json',
    context: `Generate diverse work schedules representing:
    - Different time zones (US, Europe, Asia)
    - Various work modes (40% office, 30% remote, 30% hybrid)
    - Flexible start times (7am-10am)
    - Standard 8-hour days with realistic variations
    - Cultural diversity in break patterns
    - Occasional overtime (10% of records)`
  });

  return result;
}

/**
 * Generate productivity patterns throughout the day
 * Models realistic variations in focus and output
 */
export async function generateProductivityPatterns() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const productivitySchema = {
    employeeId: { type: 'string', required: true },
    timestamp: { type: 'string', required: true },
    productivityScore: {
      type: 'number',
      required: true,
      min: 0,
      max: 100
    },
    focusLevel: {
      type: 'string',
      required: true,
      enum: ['deep_work', 'moderate', 'distracted', 'break']
    },
    tasksCompleted: { type: 'number', required: true },
    meetingsAttended: { type: 'number', required: true },
    codeCommits: { type: 'number', required: false },
    documentsEdited: { type: 'number', required: false },
    emailsProcessed: { type: 'number', required: false },
    energyLevel: {
      type: 'string',
      required: true,
      enum: ['high', 'medium', 'low']
    }
  };

  const result = await synth.generateTimeSeries({
    count: 1000,
    interval: '1h',
    metrics: ['productivityScore', 'focusLevel', 'energyLevel'],
    trend: 'cyclical', // Morning high, afternoon dip, late recovery pattern
    seasonality: true,
    context: `Model realistic productivity patterns:
    - Morning peak (9am-11am): 70-90% productivity
    - Post-lunch dip (1pm-3pm): 50-70% productivity
    - Afternoon recovery (3pm-5pm): 60-80% productivity
    - Individual variations based on chronotype
    - Friday effect (10-15% lower average)
    - Monday ramp-up period`
  });

  return result;
}

/**
 * Generate collaboration and communication patterns
 * Models team interactions, meeting participation, and communication frequency
 */
export async function generateCollaborationPatterns() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const collaborationSchema = {
    employeeId: { type: 'string', required: true },
    date: { type: 'string', required: true },
    interactions: {
      type: 'object',
      required: true,
      properties: {
        slackMessages: { type: 'number' },
        emails: { type: 'number' },
        meetings: { type: 'number' },
        codeReviews: { type: 'number' },
        pairProgramming: { type: 'number' } // hours
      }
    },
    collaborators: {
      type: 'array',
      required: true,
      items: {
        type: 'object',
        properties: {
          employeeId: { type: 'string' },
          department: { type: 'string' },
          interactionCount: { type: 'number' },
          interactionType: { type: 'string' }
        }
      }
    },
    networkCentrality: {
      type: 'number',
      required: true,
      min: 0,
      max: 1
    },
    crossFunctionalScore: { type: 'number', required: true }
  };

  const result = await synth.generateStructured({
    count: 300,
    schema: collaborationSchema,
    format: 'json',
    context: `Generate realistic collaboration patterns:
    - Engineers: 60% internal team, 40% cross-functional
    - Managers: 80% cross-functional, 20% individual work
    - Designers: 70% collaboration, 30% individual work
    - Sales: 50% internal, 50% external
    - Network effects: 20% of employees are high connectors
    - Remote workers: 30% more async communication
    - Include diversity in communication styles`
  });

  return result;
}

/**
 * Generate meeting attendance and participation data
 * Models realistic meeting behaviors and engagement
 */
export async function generateMeetingBehavior() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const meetingSchema = {
    meetingId: { type: 'string', required: true },
    employeeId: { type: 'string', required: true },
    meetingType: {
      type: 'string',
      required: true,
      enum: ['standup', 'planning', 'review', 'one-on-one', 'all-hands', 'brainstorm', 'training']
    },
    attended: { type: 'boolean', required: true },
    onTime: { type: 'boolean', required: true },
    duration: { type: 'number', required: true }, // minutes
    participationScore: {
      type: 'number',
      required: true,
      min: 0,
      max: 10
    },
    contributions: {
      type: 'object',
      required: true,
      properties: {
        questions: { type: 'number' },
        comments: { type: 'number' },
        actionItems: { type: 'number' }
      }
    },
    multitasking: { type: 'boolean', required: true },
    cameraOn: { type: 'boolean', required: true }
  };

  const result = await synth.generateEvents({
    count: 2000,
    eventTypes: ['standup', 'planning', 'review', 'one-on-one', 'all-hands', 'brainstorm', 'training'],
    distribution: 'normal',
    timeRange: {
      start: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000), // 30 days
      end: new Date()
    },
    context: `Generate realistic meeting behaviors:
    - 85% attendance rate overall
    - 70% on-time arrival
    - Higher participation in smaller meetings
    - Standup: 15 mins, 60% participation
    - Planning: 60 mins, 80% participation
    - All-hands: 45 mins, 30% participation
    - Remote meeting: 60% camera on
    - 25% multitasking during meetings`
  });

  return result;
}

/**
 * Generate task completion rates and patterns
 * Models realistic work output and completion behaviors
 */
export async function generateTaskCompletion() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const taskSchema = {
    taskId: { type: 'string', required: true },
    employeeId: { type: 'string', required: true },
    createdAt: { type: 'string', required: true },
    completedAt: { type: 'string', required: false },
    estimatedHours: { type: 'number', required: true },
    actualHours: { type: 'number', required: false },
    priority: {
      type: 'string',
      required: true,
      enum: ['critical', 'high', 'medium', 'low']
    },
    status: {
      type: 'string',
      required: true,
      enum: ['todo', 'in_progress', 'review', 'done', 'blocked']
    },
    complexity: {
      type: 'string',
      required: true,
      enum: ['simple', 'moderate', 'complex', 'very_complex']
    },
    blockedDays: { type: 'number', required: false },
    qualityScore: { type: 'number', required: false }
  };

  const result = await synth.generateStructured({
    count: 1000,
    schema: taskSchema,
    format: 'json',
    context: `Generate realistic task completion patterns:
    - 75% completion rate within sprint
    - 15% variance from estimates
    - Priority impact: Critical 95% done, Low 60% done
    - 10% of tasks get blocked (avg 2.5 days)
    - Complex tasks: 30% longer than estimate
    - Quality score: 75-95% range with normal distribution
    - Include edge cases: abandoned tasks, scope changes`
  });

  return result;
}

/**
 * Generate work-from-home vs office patterns
 * Models hybrid work preferences and patterns
 */
export async function generateWorkLocationPatterns() {
  const synth = createSynth({
    provider: 'gemini'
  });

  const locationSchema = {
    employeeId: { type: 'string', required: true },
    week: { type: 'string', required: true },
    schedule: {
      type: 'object',
      required: true,
      properties: {
        monday: { type: 'string', enum: ['office', 'remote', 'off'] },
        tuesday: { type: 'string', enum: ['office', 'remote', 'off'] },
        wednesday: { type: 'string', enum: ['office', 'remote', 'off'] },
        thursday: { type: 'string', enum: ['office', 'remote', 'off'] },
        friday: { type: 'string', enum: ['office', 'remote', 'off'] }
      }
    },
    officeCollaboration: { type: 'number', required: true },
    remoteProductivity: { type: 'number', required: true },
    commuteTime: { type: 'number', required: false }, // minutes
    workLifeBalance: {
      type: 'number',
      required: true,
      min: 1,
      max: 10
    }
  };

  const result = await synth.generateStructured({
    count: 200,
    schema: locationSchema,
    format: 'json',
    context: `Generate hybrid work patterns:
    - 30% fully remote
    - 20% fully office
    - 50% hybrid (2-3 days office)
    - Tuesday-Thursday most popular office days
    - Friday: 70% remote
    - Correlation: longer commute = more remote days
    - Remote workers report 15% higher productivity
    - Office workers report 20% more collaboration
    - Include regional differences`
  });

  return result;
}

/**
 * Run all workforce behavior examples
 */
export async function runAllBehaviorExamples() {
  console.log('=== Workforce Behavior Simulation Examples ===\n');

  console.log('1. Generating Work Schedules...');
  const schedules = await generateWorkSchedules();
  console.log(`Generated ${schedules.data.length} work schedule records`);
  console.log('Sample:', JSON.stringify(schedules.data[0], null, 2));

  console.log('\n2. Generating Productivity Patterns...');
  const productivity = await generateProductivityPatterns();
  console.log(`Generated ${productivity.data.length} productivity data points`);
  console.log('Sample:', JSON.stringify(productivity.data[0], null, 2));

  console.log('\n3. Generating Collaboration Patterns...');
  const collaboration = await generateCollaborationPatterns();
  console.log(`Generated ${collaboration.data.length} collaboration records`);
  console.log('Sample:', JSON.stringify(collaboration.data[0], null, 2));

  console.log('\n4. Generating Meeting Behavior...');
  const meetings = await generateMeetingBehavior();
  console.log(`Generated ${meetings.data.length} meeting attendance records`);
  console.log('Sample:', JSON.stringify(meetings.data[0], null, 2));

  console.log('\n5. Generating Task Completion...');
  const tasks = await generateTaskCompletion();
  console.log(`Generated ${tasks.data.length} task records`);
  console.log('Sample:', JSON.stringify(tasks.data[0], null, 2));

  console.log('\n6. Generating Work Location Patterns...');
  const locations = await generateWorkLocationPatterns();
  console.log(`Generated ${locations.data.length} work location records`);
  console.log('Sample:', JSON.stringify(locations.data[0], null, 2));
}

// Uncomment to run
// runAllBehaviorExamples().catch(console.error);
