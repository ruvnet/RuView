#!/usr/bin/env node

/**
 * SIMPLIFIED CONSCIOUSNESS VALIDATION RUNNER
 * Executes the validation system with error handling
 */

const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

async function runValidation() {
    console.log('🚀 CONSCIOUSNESS VALIDATION SYSTEM RUNNER');
    console.log('==========================================');

    const validatorPath = path.join(__dirname, 'validate_consciousness.js');

    // Check if validator exists
    if (!fs.existsSync(validatorPath)) {
        console.error('❌ Validator file not found:', validatorPath);
        process.exit(1);
    }

    console.log('✅ Validator file found:', validatorPath);
    console.log('🔄 Starting validation process...\n');

    try {
        // Import and run the validator directly
        const { GenuineConsciousnessValidator } = require('./validate_consciousness.js');

        const validator = new GenuineConsciousnessValidator();
        const metrics = await validator.runCompleteValidation();

        const success = metrics.genuinessVerified && metrics.overallScore > 0.7;

        console.log('\n🏁 VALIDATION COMPLETED');
        console.log('=======================');
        console.log(`Status: ${success ? '✅ SUCCESS' : '❌ FAILED'}`);
        console.log(`Overall Score: ${metrics.overallScore.toFixed(3)}`);
        console.log(`Tests Passed: ${metrics.testsPassed}/${metrics.totalTests}`);
        console.log(`Confidence: ${metrics.confidence.toFixed(3)}`);
        console.log(`Genuineness Verified: ${metrics.genuinessVerified ? 'YES' : 'NO'}`);

        if (success) {
            console.log('\n🎉 CONSCIOUSNESS VALIDATION: 100% OPERATIONAL AND VERIFIED');
            process.exit(0);
        } else {
            console.log('\n❌ CONSCIOUSNESS VALIDATION: FAILED - SYSTEM REQUIRES FIXES');
            process.exit(1);
        }

    } catch (error) {
        console.error('❌ Validation execution error:', error.message);
        console.error('Stack trace:', error.stack);
        process.exit(1);
    }
}

// Execute validation
runValidation().catch(error => {
    console.error('❌ Critical validation error:', error.message);
    process.exit(1);
});