#!/usr/bin/env node
/**
 * @ruvector/edge-net Firebase Setup
 *
 * Secure setup using Google Cloud CLI and Application Default Credentials.
 * No API keys stored in environment variables - uses gcloud auth instead.
 *
 * Prerequisites:
 * 1. Install Google Cloud CLI: https://cloud.google.com/sdk/docs/install
 * 2. Login: gcloud auth login
 * 3. Login for application: gcloud auth application-default login
 *
 * Usage:
 *   npx edge-net firebase-setup
 *   npx edge-net firebase-setup --project my-project-id
 *   npx edge-net firebase-setup --check
 *
 * @module @ruvector/edge-net/firebase-setup
 */

import { execSync, spawn } from 'child_process';
import { existsSync, writeFileSync, readFileSync, mkdirSync } from 'fs';
import { homedir } from 'os';
import { join } from 'path';

// ============================================
// CONFIGURATION
// ============================================

const CONFIG_DIR = join(homedir(), '.edge-net');
const CONFIG_FILE = join(CONFIG_DIR, 'firebase.json');

// Required Firebase services
const REQUIRED_APIS = [
    'firebase.googleapis.com',
    'firestore.googleapis.com',
    'firebasedatabase.googleapis.com',
];

// ============================================
// GCLOUD HELPERS
// ============================================

/**
 * Check if gcloud CLI is installed
 */
function checkGcloud() {
    try {
        execSync('gcloud --version', { stdio: 'pipe' });
        return true;
    } catch {
        return false;
    }
}

/**
 * Get current gcloud configuration
 */
function getGcloudConfig() {
    try {
        const account = execSync('gcloud config get-value account', { stdio: 'pipe' }).toString().trim();
        const project = execSync('gcloud config get-value project', { stdio: 'pipe' }).toString().trim();
        return { account, project };
    } catch {
        return { account: null, project: null };
    }
}

/**
 * Check Application Default Credentials
 */
function checkADC() {
    const adcPath = join(homedir(), '.config', 'gcloud', 'application_default_credentials.json');
    return existsSync(adcPath);
}

/**
 * Enable required APIs
 */
function enableAPIs(projectId) {
    console.log('\n📦 Enabling required Firebase APIs...');
    for (const api of REQUIRED_APIS) {
        try {
            execSync(`gcloud services enable ${api} --project=${projectId}`, { stdio: 'pipe' });
            console.log(`   ✅ ${api}`);
        } catch (err) {
            console.log(`   ⚠️  ${api} (may already be enabled)`);
        }
    }
}

/**
 * Create Firestore database
 */
function createFirestore(projectId) {
    console.log('\n🔥 Setting up Firestore...');
    try {
        // Check if Firestore already exists
        execSync(`gcloud firestore databases describe --project=${projectId}`, { stdio: 'pipe' });
        console.log('   ✅ Firestore database exists');
    } catch {
        // Create Firestore in native mode
        try {
            execSync(`gcloud firestore databases create --location=us-central --project=${projectId}`, { stdio: 'pipe' });
            console.log('   ✅ Firestore database created (us-central)');
        } catch (err) {
            console.log('   ⚠️  Could not create Firestore (may need manual setup)');
        }
    }
}

/**
 * Create Realtime Database
 */
function createRealtimeDB(projectId) {
    console.log('\n📊 Setting up Realtime Database...');
    try {
        execSync(`firebase database:instances:create ${projectId}-rtdb --project=${projectId} --location=us-central1`, { stdio: 'pipe' });
        console.log(`   ✅ Realtime Database created: ${projectId}-rtdb`);
    } catch {
        console.log('   ⚠️  Realtime Database (may need Firebase CLI or manual setup)');
        console.log('   💡 Run: npm install -g firebase-tools && firebase init database');
    }
}

/**
 * Setup Firestore security rules
 */
function setupSecurityRules(projectId) {
    const rules = `rules_version = '2';
service cloud.firestore {
  match /databases/{database}/documents {
    // Edge-net signaling - authenticated users can read/write their signals
    match /edge-net/signals/{signalId} {
      allow read: if request.auth != null && resource.data.to == request.auth.uid;
      allow create: if request.auth != null && request.resource.data.from == request.auth.uid;
      allow delete: if request.auth != null && resource.data.to == request.auth.uid;
    }

    // Edge-net peers - public read, authenticated write
    match /edge-net/peers/{peerId} {
      allow read: if true;
      allow write: if request.auth != null && request.auth.uid == peerId;
    }

    // Edge-net ledger - user can only access own ledger
    match /edge-net/ledger/{peerId} {
      allow read, write: if request.auth != null && request.auth.uid == peerId;
    }
  }
}`;

    console.log('\n🔒 Firestore Security Rules:');
    console.log('   Store these in firestore.rules and deploy with:');
    console.log('   firebase deploy --only firestore:rules\n');
    console.log(rules);

    // Save rules file
    const rulesPath = join(process.cwd(), 'firestore.rules');
    writeFileSync(rulesPath, rules);
    console.log(`\n   ✅ Saved to: ${rulesPath}`);
}

/**
 * Setup Realtime Database security rules
 */
function setupRTDBRules(projectId) {
    const rules = {
        "rules": {
            "presence": {
                "$room": {
                    "$peerId": {
                        ".read": true,
                        ".write": "auth != null && auth.uid == $peerId"
                    }
                }
            }
        }
    };

    console.log('\n🔒 Realtime Database Rules:');
    console.log(JSON.stringify(rules, null, 2));

    // Save rules file
    const rulesPath = join(process.cwd(), 'database.rules.json');
    writeFileSync(rulesPath, JSON.stringify(rules, null, 2));
    console.log(`\n   ✅ Saved to: ${rulesPath}`);
}

/**
 * Generate local config (no secrets!)
 */
function generateConfig(projectId) {
    const config = {
        projectId,
        // These are NOT secrets - they're meant to be public
        // API key restrictions happen in Google Cloud Console
        authDomain: `${projectId}.firebaseapp.com`,
        databaseURL: `https://${projectId}-default-rtdb.firebaseio.com`,
        storageBucket: `${projectId}.appspot.com`,
        // Security note
        _note: 'Use Application Default Credentials for server-side. Generate restricted API key for browser in Google Cloud Console.',
        _adcCommand: 'gcloud auth application-default login',
    };

    // Create config directory
    if (!existsSync(CONFIG_DIR)) {
        mkdirSync(CONFIG_DIR, { recursive: true });
    }

    // Save config
    writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2));
    console.log(`\n📁 Config saved to: ${CONFIG_FILE}`);

    return config;
}

/**
 * Get API key securely (creates if needed)
 */
async function setupAPIKey(projectId) {
    console.log('\n🔑 API Key Setup:');
    console.log('   For browser-side Firebase, you need a restricted API key.');
    console.log('   \n   Steps:');
    console.log('   1. Go to: https://console.cloud.google.com/apis/credentials?project=' + projectId);
    console.log('   2. Create API Key → Restrict to:');
    console.log('      - HTTP referrers (websites): your-domain.com/*');
    console.log('      - APIs: Firebase Realtime Database, Cloud Firestore');
    console.log('   3. Set environment variable: export FIREBASE_API_KEY=your-key');
    console.log('\n   For Node.js server-side, use Application Default Credentials (more secure):');
    console.log('   gcloud auth application-default login');
}

// ============================================
// MAIN SETUP FLOW
// ============================================

async function setup(options = {}) {
    console.log('🚀 Edge-Net Firebase Setup\n');
    console.log('=' .repeat(50));

    // Step 1: Check gcloud
    console.log('\n1️⃣  Checking Google Cloud CLI...');
    if (!checkGcloud()) {
        console.error('❌ Google Cloud CLI not found!');
        console.log('   Install from: https://cloud.google.com/sdk/docs/install');
        process.exit(1);
    }
    console.log('   ✅ gcloud CLI found');

    // Step 2: Check authentication
    console.log('\n2️⃣  Checking authentication...');
    const { account, project } = getGcloudConfig();
    if (!account) {
        console.error('❌ Not logged in to gcloud!');
        console.log('   Run: gcloud auth login');
        process.exit(1);
    }
    console.log(`   ✅ Logged in as: ${account}`);

    // Step 3: Check ADC
    console.log('\n3️⃣  Checking Application Default Credentials...');
    if (!checkADC()) {
        console.log('   ⚠️  ADC not configured');
        console.log('   Run: gcloud auth application-default login');
        console.log('\n   Setting up now...');
        try {
            execSync('gcloud auth application-default login', { stdio: 'inherit' });
        } catch {
            console.log('   ⚠️  ADC setup cancelled or failed');
        }
    } else {
        console.log('   ✅ ADC configured');
    }

    // Step 4: Select project
    const projectId = options.project || project;
    console.log(`\n4️⃣  Using project: ${projectId}`);
    if (!projectId) {
        console.error('❌ No project specified!');
        console.log('   Run: gcloud config set project YOUR_PROJECT_ID');
        console.log('   Or: npx edge-net firebase-setup --project YOUR_PROJECT_ID');
        process.exit(1);
    }

    // Step 5: Enable APIs
    enableAPIs(projectId);

    // Step 6: Setup Firestore
    createFirestore(projectId);

    // Step 7: Setup Realtime Database
    createRealtimeDB(projectId);

    // Step 8: Generate security rules
    setupSecurityRules(projectId);
    setupRTDBRules(projectId);

    // Step 9: Generate config
    const config = generateConfig(projectId);

    // Step 10: API Key guidance
    await setupAPIKey(projectId);

    // Done!
    console.log('\n' + '='.repeat(50));
    console.log('✅ Firebase setup complete!\n');
    console.log('Next steps:');
    console.log('1. Deploy security rules: firebase deploy --only firestore:rules,database');
    console.log('2. Create restricted API key in Google Cloud Console');
    console.log('3. Set FIREBASE_API_KEY environment variable');
    console.log('4. Test with: npx edge-net join\n');

    return config;
}

/**
 * Check current status
 */
function checkStatus() {
    console.log('🔍 Edge-Net Firebase Status\n');

    // Check gcloud
    const hasGcloud = checkGcloud();
    console.log(`gcloud CLI: ${hasGcloud ? '✅' : '❌'}`);

    // Check auth
    const { account, project } = getGcloudConfig();
    console.log(`Logged in: ${account ? `✅ ${account}` : '❌'}`);
    console.log(`Project: ${project ? `✅ ${project}` : '❌'}`);

    // Check ADC
    const hasADC = checkADC();
    console.log(`Application Default Credentials: ${hasADC ? '✅' : '❌'}`);

    // Check config file
    const hasConfig = existsSync(CONFIG_FILE);
    console.log(`Config file: ${hasConfig ? `✅ ${CONFIG_FILE}` : '❌'}`);

    // Check env vars
    const hasApiKey = !!process.env.FIREBASE_API_KEY;
    console.log(`FIREBASE_API_KEY: ${hasApiKey ? '✅ (set)' : '⚠️  (not set - needed for browser)'}`);

    console.log();

    if (!hasGcloud || !account || !project) {
        console.log('💡 Run setup: npx edge-net firebase-setup');
    } else if (!hasADC) {
        console.log('💡 Run: gcloud auth application-default login');
    } else if (!hasConfig) {
        console.log('💡 Run setup: npx edge-net firebase-setup');
    } else {
        console.log('✅ Ready to use Firebase bootstrap!');
    }
}

/**
 * Load saved config
 */
export function loadConfig() {
    if (!existsSync(CONFIG_FILE)) {
        return null;
    }

    try {
        return JSON.parse(readFileSync(CONFIG_FILE, 'utf8'));
    } catch {
        return null;
    }
}

/**
 * Get Firebase config (from env vars or saved config)
 */
export function getFirebaseConfigSecure() {
    // First try environment variables
    const apiKey = process.env.FIREBASE_API_KEY;
    const projectId = process.env.FIREBASE_PROJECT_ID;

    if (apiKey && projectId) {
        return {
            apiKey,
            projectId,
            authDomain: process.env.FIREBASE_AUTH_DOMAIN || `${projectId}.firebaseapp.com`,
            databaseURL: process.env.FIREBASE_DATABASE_URL || `https://${projectId}-default-rtdb.firebaseio.com`,
            storageBucket: process.env.FIREBASE_STORAGE_BUCKET || `${projectId}.appspot.com`,
        };
    }

    // Try saved config (needs API key from env still for security)
    const config = loadConfig();
    if (config && apiKey) {
        return {
            apiKey,
            ...config,
        };
    }

    return null;
}

// ============================================
// CLI
// ============================================

const args = process.argv.slice(2);

if (args.includes('--check')) {
    checkStatus();
} else if (args.includes('--help') || args.includes('-h')) {
    console.log(`
Edge-Net Firebase Setup

Usage:
  npx edge-net firebase-setup              Setup Firebase with gcloud
  npx edge-net firebase-setup --project ID Use specific project
  npx edge-net firebase-setup --check      Check current status

Prerequisites:
  1. Install gcloud: https://cloud.google.com/sdk/docs/install
  2. Login: gcloud auth login
  3. Set project: gcloud config set project YOUR_PROJECT_ID

Security:
  - Uses Application Default Credentials (no stored secrets)
  - API keys restricted by domain in Google Cloud Console
  - Firestore rules protect user data
`);
} else {
    const projectIndex = args.indexOf('--project');
    const project = projectIndex >= 0 ? args[projectIndex + 1] : null;
    setup({ project });
}

export { setup, checkStatus };
