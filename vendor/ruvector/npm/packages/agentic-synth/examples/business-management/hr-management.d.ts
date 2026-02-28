/**
 * Human Resources Management Data Generation
 * Simulates Workday, SAP SuccessFactors, and Oracle HCM Cloud scenarios
 */
/**
 * Generate Workday Employee Profiles
 */
export declare function generateEmployeeProfiles(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate SAP SuccessFactors Recruitment Pipeline
 */
export declare function generateRecruitmentPipeline(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Oracle HCM Performance Reviews
 */
export declare function generatePerformanceReviews(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Workday Payroll Data
 */
export declare function generatePayrollData(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Time Tracking and Attendance Data (time-series)
 */
export declare function generateTimeAttendance(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate Training and Development Records
 */
export declare function generateTrainingRecords(count?: number): Promise<import("../../src/types.js").GenerationResult<unknown>>;
/**
 * Generate complete HR dataset in parallel
 */
export declare function generateCompleteHRDataset(): Promise<{
    employees: unknown[];
    recruitment: unknown[];
    performanceReviews: unknown[];
    payroll: unknown[];
    timeAttendance: unknown[];
    training: unknown[];
    metadata: {
        totalRecords: number;
        generatedAt: string;
    };
}>;
declare const _default: {
    generateEmployeeProfiles: typeof generateEmployeeProfiles;
    generateRecruitmentPipeline: typeof generateRecruitmentPipeline;
    generatePerformanceReviews: typeof generatePerformanceReviews;
    generatePayrollData: typeof generatePayrollData;
    generateTimeAttendance: typeof generateTimeAttendance;
    generateTrainingRecords: typeof generateTrainingRecords;
    generateCompleteHRDataset: typeof generateCompleteHRDataset;
};
export default _default;
//# sourceMappingURL=hr-management.d.ts.map