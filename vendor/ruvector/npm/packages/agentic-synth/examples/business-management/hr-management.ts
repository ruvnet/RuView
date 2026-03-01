/**
 * Human Resources Management Data Generation
 * Simulates Workday, SAP SuccessFactors, and Oracle HCM Cloud scenarios
 */

import { createSynth } from '../../src/index.js';

// Workday Employee Profile Schema
const employeeProfileSchema = {
  employeeId: { type: 'string', required: true },
  employeeNumber: { type: 'string', required: true },
  firstName: { type: 'string', required: true },
  middleName: { type: 'string', required: false },
  lastName: { type: 'string', required: true },
  preferredName: { type: 'string', required: false },
  dateOfBirth: { type: 'string', required: true },
  gender: { type: 'string', required: true },
  maritalStatus: { type: 'string', required: false },
  nationality: { type: 'string', required: true },
  ethnicity: { type: 'string', required: false },
  contactInfo: { type: 'object', required: true, properties: {
    personalEmail: { type: 'string' },
    workEmail: { type: 'string' },
    personalPhone: { type: 'string' },
    workPhone: { type: 'string' },
    mobile: { type: 'string' }
  }},
  address: { type: 'object', required: true, properties: {
    street1: { type: 'string' },
    street2: { type: 'string' },
    city: { type: 'string' },
    state: { type: 'string' },
    postalCode: { type: 'string' },
    country: { type: 'string' }
  }},
  employment: { type: 'object', required: true, properties: {
    hireDate: { type: 'string' },
    originalHireDate: { type: 'string' },
    employmentType: { type: 'string' },
    employmentStatus: { type: 'string' },
    workSchedule: { type: 'string' },
    fullTimeEquivalent: { type: 'number' },
    terminationDate: { type: 'string' },
    terminationReason: { type: 'string' }
  }},
  jobInfo: { type: 'object', required: true, properties: {
    jobTitle: { type: 'string' },
    jobCode: { type: 'string' },
    jobFamily: { type: 'string' },
    jobLevel: { type: 'string' },
    department: { type: 'string' },
    division: { type: 'string' },
    businessUnit: { type: 'string' },
    costCenter: { type: 'string' },
    location: { type: 'string' },
    workSite: { type: 'string' }
  }},
  reportingStructure: { type: 'object', required: true, properties: {
    managerId: { type: 'string' },
    managerName: { type: 'string' },
    dotted LineManagerId: { type: 'string' },
    dottedLineManagerName: { type: 'string' },
    seniorManagerId: { type: 'string' },
    seniorManagerName: { type: 'string' }
  }},
  compensation: { type: 'object', required: true, properties: {
    baseSalary: { type: 'number' },
    currency: { type: 'string' },
    payGrade: { type: 'string' },
    payGroup: { type: 'string' },
    payFrequency: { type: 'string' },
    overtimeEligible: { type: 'boolean' },
    bonusTarget: { type: 'number' },
    equityGrants: { type: 'array' }
  }},
  benefits: { type: 'object', required: false, properties: {
    healthPlan: { type: 'string' },
    dentalPlan: { type: 'string' },
    visionPlan: { type: 'string' },
    retirement401k: { type: 'boolean' },
    stockPurchasePlan: { type: 'boolean' }
  }},
  skills: { type: 'array', required: false },
  certifications: { type: 'array', required: false },
  education: { type: 'array', required: false, items: {
    degree: { type: 'string' },
    institution: { type: 'string' },
    major: { type: 'string' },
    graduationYear: { type: 'number' }
  }}
};

// SAP SuccessFactors Recruitment Pipeline Schema
const recruitmentPipelineSchema = {
  requisitionId: { type: 'string', required: true },
  jobPostingId: { type: 'string', required: true },
  requisitionTitle: { type: 'string', required: true },
  department: { type: 'string', required: true },
  location: { type: 'string', required: true },
  hiringManager: { type: 'object', required: true, properties: {
    employeeId: { type: 'string' },
    name: { type: 'string' },
    email: { type: 'string' }
  }},
  recruiter: { type: 'object', required: true, properties: {
    employeeId: { type: 'string' },
    name: { type: 'string' },
    email: { type: 'string' }
  }},
  jobDetails: { type: 'object', required: true, properties: {
    jobFamily: { type: 'string' },
    jobLevel: { type: 'string' },
    employmentType: { type: 'string' },
    experienceRequired: { type: 'string' },
    educationRequired: { type: 'string' },
    skillsRequired: { type: 'array' }
  }},
  compensation: { type: 'object', required: true, properties: {
    salaryRangeMin: { type: 'number' },
    salaryRangeMax: { type: 'number' },
    currency: { type: 'string' },
    bonusEligible: { type: 'boolean' },
    equityEligible: { type: 'boolean' }
  }},
  openDate: { type: 'string', required: true },
  targetFillDate: { type: 'string', required: true },
  status: { type: 'string', required: true },
  candidates: { type: 'array', required: true, items: {
    candidateId: { type: 'string' },
    candidateName: { type: 'string' },
    email: { type: 'string' },
    phone: { type: 'string' },
    source: { type: 'string' },
    appliedDate: { type: 'string' },
    stage: { type: 'string' },
    status: { type: 'string' },
    rating: { type: 'number' },
    interviews: { type: 'array' },
    offer: { type: 'object' }
  }},
  metrics: { type: 'object', required: true, properties: {
    totalCandidates: { type: 'number' },
    screenedCandidates: { type: 'number' },
    interviewedCandidates: { type: 'number' },
    offersExtended: { type: 'number' },
    offersAccepted: { type: 'number' },
    daysToFill: { type: 'number' },
    timeToHire: { type: 'number' }
  }}
};

// Oracle HCM Performance Review Schema
const performanceReviewSchema = {
  reviewId: { type: 'string', required: true },
  reviewPeriod: { type: 'object', required: true, properties: {
    startDate: { type: 'string' },
    endDate: { type: 'string' },
    reviewType: { type: 'string' },
    reviewCycle: { type: 'string' }
  }},
  employee: { type: 'object', required: true, properties: {
    employeeId: { type: 'string' },
    employeeName: { type: 'string' },
    jobTitle: { type: 'string' },
    department: { type: 'string' }
  }},
  reviewer: { type: 'object', required: true, properties: {
    reviewerId: { type: 'string' },
    reviewerName: { type: 'string' },
    relationship: { type: 'string' }
  }},
  goals: { type: 'array', required: true, items: {
    goalId: { type: 'string' },
    goalName: { type: 'string' },
    goalDescription: { type: 'string' },
    goalType: { type: 'string' },
    weight: { type: 'number' },
    targetDate: { type: 'string' },
    status: { type: 'string' },
    achievement: { type: 'number' },
    rating: { type: 'string' }
  }},
  competencies: { type: 'array', required: true, items: {
    competencyId: { type: 'string' },
    competencyName: { type: 'string' },
    expectedLevel: { type: 'string' },
    actualLevel: { type: 'string' },
    rating: { type: 'number' },
    evidence: { type: 'string' }
  }},
  overallRating: { type: 'object', required: true, properties: {
    rating: { type: 'number' },
    ratingLabel: { type: 'string' },
    percentile: { type: 'number' },
    distribution: { type: 'string' }
  }},
  feedback: { type: 'object', required: true, properties: {
    strengths: { type: 'array' },
    areasForImprovement: { type: 'array' },
    managerComments: { type: 'string' },
    employeeComments: { type: 'string' }
  }},
  developmentPlan: { type: 'array', required: false, items: {
    action: { type: 'string' },
    targetDate: { type: 'string' },
    status: { type: 'string' }
  }},
  compensation: { type: 'object', required: false, properties: {
    salaryIncreasePercent: { type: 'number' },
    bonusPercent: { type: 'number' },
    promotionRecommended: { type: 'boolean' },
    newJobTitle: { type: 'string' }
  }},
  status: { type: 'string', required: true },
  submittedDate: { type: 'string', required: false },
  approvedDate: { type: 'string', required: false }
};

// Workday Payroll Data Schema
const payrollDataSchema = {
  payrollId: { type: 'string', required: true },
  payPeriod: { type: 'object', required: true, properties: {
    periodStartDate: { type: 'string' },
    periodEndDate: { type: 'string' },
    payDate: { type: 'string' },
    periodNumber: { type: 'number' },
    fiscalYear: { type: 'number' }
  }},
  employee: { type: 'object', required: true, properties: {
    employeeId: { type: 'string' },
    employeeName: { type: 'string' },
    employeeNumber: { type: 'string' },
    department: { type: 'string' },
    costCenter: { type: 'string' }
  }},
  earnings: { type: 'array', required: true, items: {
    earningCode: { type: 'string' },
    earningDescription: { type: 'string' },
    hours: { type: 'number' },
    rate: { type: 'number' },
    amount: { type: 'number' },
    earningCategory: { type: 'string' }
  }},
  deductions: { type: 'array', required: true, items: {
    deductionCode: { type: 'string' },
    deductionDescription: { type: 'string' },
    amount: { type: 'number' },
    deductionCategory: { type: 'string' },
    employerContribution: { type: 'number' }
  }},
  taxes: { type: 'array', required: true, items: {
    taxCode: { type: 'string' },
    taxDescription: { type: 'string' },
    taxableWages: { type: 'number' },
    taxAmount: { type: 'number' },
    taxAuthority: { type: 'string' }
  }},
  summary: { type: 'object', required: true, properties: {
    grossPay: { type: 'number' },
    totalDeductions: { type: 'number' },
    totalTaxes: { type: 'number' },
    netPay: { type: 'number' },
    currency: { type: 'string' }
  }},
  paymentMethod: { type: 'object', required: true, properties: {
    method: { type: 'string' },
    bankName: { type: 'string' },
    accountNumber: { type: 'string' },
    routingNumber: { type: 'string' }
  }},
  yearToDate: { type: 'object', required: true, properties: {
    ytdGrossPay: { type: 'number' },
    ytdDeductions: { type: 'number' },
    ytdTaxes: { type: 'number' },
    ytdNetPay: { type: 'number' }
  }}
};

// Time Tracking and Attendance Schema
const timeAttendanceSchema = {
  recordId: { type: 'string', required: true },
  employee: { type: 'object', required: true, properties: {
    employeeId: { type: 'string' },
    employeeName: { type: 'string' },
    department: { type: 'string' }
  }},
  date: { type: 'string', required: true },
  shift: { type: 'object', required: true, properties: {
    shiftId: { type: 'string' },
    shiftName: { type: 'string' },
    scheduledStart: { type: 'string' },
    scheduledEnd: { type: 'string' },
    breakDuration: { type: 'number' }
  }},
  actual: { type: 'object', required: true, properties: {
    clockIn: { type: 'string' },
    clockOut: { type: 'string' },
    breakStart: { type: 'string' },
    breakEnd: { type: 'string' },
    totalHours: { type: 'number' }
  }},
  hoursBreakdown: { type: 'object', required: true, properties: {
    regularHours: { type: 'number' },
    overtimeHours: { type: 'number' },
    doubleTimeHours: { type: 'number' },
    ptoHours: { type: 'number' },
    sickHours: { type: 'number' },
    holidayHours: { type: 'number' }
  }},
  attendance: { type: 'object', required: true, properties: {
    status: { type: 'string' },
    late: { type: 'boolean' },
    lateMinutes: { type: 'number' },
    earlyDeparture: { type: 'boolean' },
    absent: { type: 'boolean' },
    excused: { type: 'boolean' }
  }},
  location: { type: 'object', required: false, properties: {
    site: { type: 'string' },
    gpsCoordinates: { type: 'object' }
  }},
  approver: { type: 'object', required: false, properties: {
    approverId: { type: 'string' },
    approverName: { type: 'string' },
    approvedDate: { type: 'string' }
  }}
};

// Training and Development Schema
const trainingDevelopmentSchema = {
  trainingId: { type: 'string', required: true },
  employee: { type: 'object', required: true, properties: {
    employeeId: { type: 'string' },
    employeeName: { type: 'string' },
    department: { type: 'string' },
    jobTitle: { type: 'string' }
  }},
  course: { type: 'object', required: true, properties: {
    courseId: { type: 'string' },
    courseName: { type: 'string' },
    courseType: { type: 'string' },
    provider: { type: 'string' },
    deliveryMethod: { type: 'string' },
    duration: { type: 'number' },
    cost: { type: 'number' }
  }},
  schedule: { type: 'object', required: true, properties: {
    startDate: { type: 'string' },
    endDate: { type: 'string' },
    completionDate: { type: 'string' },
    expirationDate: { type: 'string' }
  }},
  status: { type: 'string', required: true },
  completion: { type: 'object', required: false, properties: {
    completed: { type: 'boolean' },
    score: { type: 'number' },
    grade: { type: 'string' },
    certificateIssued: { type: 'boolean' },
    certificateNumber: { type: 'string' }
  }},
  evaluation: { type: 'object', required: false, properties: {
    satisfaction: { type: 'number' },
    relevance: { type: 'number' },
    effectiveness: { type: 'number' },
    feedback: { type: 'string' }
  }},
  linkedCompetencies: { type: 'array', required: false },
  developmentPlanId: { type: 'string', required: false },
  requiredFor: { type: 'object', required: false, properties: {
    compliance: { type: 'boolean' },
    certification: { type: 'boolean' },
    promotion: { type: 'boolean' }
  }}
};

/**
 * Generate Workday Employee Profiles
 */
export async function generateEmployeeProfiles(count: number = 100) {
  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  console.log(`Generating ${count} employee profiles...`);

  const result = await synth.generateStructured({
    count,
    schema: employeeProfileSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} profiles in ${result.metadata.duration}ms`);
  console.log('Sample profile:', result.data[0]);

  return result;
}

/**
 * Generate SAP SuccessFactors Recruitment Pipeline
 */
export async function generateRecruitmentPipeline(count: number = 25) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} recruitment requisitions...`);

  const result = await synth.generateStructured({
    count,
    schema: recruitmentPipelineSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} requisitions in ${result.metadata.duration}ms`);
  console.log('Sample requisition:', result.data[0]);

  return result;
}

/**
 * Generate Oracle HCM Performance Reviews
 */
export async function generatePerformanceReviews(count: number = 75) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} performance reviews...`);

  const result = await synth.generateStructured({
    count,
    schema: performanceReviewSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} reviews in ${result.metadata.duration}ms`);
  console.log('Sample review:', result.data[0]);

  return result;
}

/**
 * Generate Workday Payroll Data
 */
export async function generatePayrollData(count: number = 500) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} payroll records...`);

  const result = await synth.generateStructured({
    count,
    schema: payrollDataSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} payroll records in ${result.metadata.duration}ms`);
  console.log('Sample payroll:', result.data[0]);

  return result;
}

/**
 * Generate Time Tracking and Attendance Data (time-series)
 */
export async function generateTimeAttendance(count: number = 1000) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} time & attendance records...`);

  const result = await synth.generateTimeSeries({
    count,
    interval: '1d',
    metrics: ['hoursWorked', 'overtimeHours', 'attendance'],
    trend: 'stable',
    seasonality: true
  });

  console.log(`Generated ${result.data.length} records in ${result.metadata.duration}ms`);
  console.log('Sample record:', result.data[0]);

  return result;
}

/**
 * Generate Training and Development Records
 */
export async function generateTrainingRecords(count: number = 200) {
  const synth = createSynth({
    provider: 'gemini'
  });

  console.log(`Generating ${count} training records...`);

  const result = await synth.generateStructured({
    count,
    schema: trainingDevelopmentSchema,
    format: 'json'
  });

  console.log(`Generated ${result.data.length} training records in ${result.metadata.duration}ms`);
  console.log('Sample record:', result.data[0]);

  return result;
}

/**
 * Generate complete HR dataset in parallel
 */
export async function generateCompleteHRDataset() {
  const synth = createSynth({
    provider: 'gemini',
    cacheStrategy: 'memory'
  });

  console.log('Generating complete HR dataset in parallel...');
  console.time('Total HR generation');

  const [employees, recruitment, performance, payroll, timeAttendance, training] =
    await Promise.all([
      generateEmployeeProfiles(100),
      generateRecruitmentPipeline(20),
      generatePerformanceReviews(50),
      generatePayrollData(200),
      generateTimeAttendance(500),
      generateTrainingRecords(100)
    ]);

  console.timeEnd('Total HR generation');

  return {
    employees: employees.data,
    recruitment: recruitment.data,
    performanceReviews: performance.data,
    payroll: payroll.data,
    timeAttendance: timeAttendance.data,
    training: training.data,
    metadata: {
      totalRecords: employees.data.length + recruitment.data.length +
                   performance.data.length + payroll.data.length +
                   timeAttendance.data.length + training.data.length,
      generatedAt: new Date().toISOString()
    }
  };
}

// Example usage
async function runHRExamples() {
  console.log('=== HR Management Data Generation Examples ===\n');

  // Example 1: Employee Profiles
  console.log('1. Employee Profiles (Workday)');
  await generateEmployeeProfiles(10);

  // Example 2: Recruitment Pipeline
  console.log('\n2. Recruitment Pipeline (SuccessFactors)');
  await generateRecruitmentPipeline(5);

  // Example 3: Performance Reviews
  console.log('\n3. Performance Reviews (Oracle HCM)');
  await generatePerformanceReviews(10);

  // Example 4: Payroll Data
  console.log('\n4. Payroll Data (Workday)');
  await generatePayrollData(25);

  // Example 5: Time & Attendance
  console.log('\n5. Time & Attendance');
  await generateTimeAttendance(50);

  // Example 6: Training Records
  console.log('\n6. Training & Development');
  await generateTrainingRecords(20);

  // Example 7: Complete HR dataset
  console.log('\n7. Complete HR Dataset (Parallel)');
  const completeDataset = await generateCompleteHRDataset();
  console.log('Total records generated:', completeDataset.metadata.totalRecords);
}

// Uncomment to run
// runHRExamples().catch(console.error);

export default {
  generateEmployeeProfiles,
  generateRecruitmentPipeline,
  generatePerformanceReviews,
  generatePayrollData,
  generateTimeAttendance,
  generateTrainingRecords,
  generateCompleteHRDataset
};
