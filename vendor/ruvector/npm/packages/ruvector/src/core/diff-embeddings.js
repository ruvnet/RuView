"use strict";
/**
 * Diff Embeddings - Semantic encoding of git diffs
 *
 * Generates embeddings for code changes to enable:
 * - Change classification (feature, bugfix, refactor)
 * - Similar change detection
 * - Risk assessment
 * - Review prioritization
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseDiff = parseDiff;
exports.classifyChange = classifyChange;
exports.calculateRiskScore = calculateRiskScore;
exports.analyzeFileDiff = analyzeFileDiff;
exports.getCommitDiff = getCommitDiff;
exports.getStagedDiff = getStagedDiff;
exports.getUnstagedDiff = getUnstagedDiff;
exports.analyzeCommit = analyzeCommit;
exports.findSimilarCommits = findSimilarCommits;
const child_process_1 = require("child_process");
const onnx_embedder_1 = require("./onnx-embedder");
/**
 * Parse a unified diff into hunks
 */
function parseDiff(diff) {
    const hunks = [];
    const lines = diff.split('\n');
    let currentFile = '';
    let currentHunk = null;
    for (const line of lines) {
        // File header
        if (line.startsWith('diff --git')) {
            const match = line.match(/diff --git a\/(.+) b\/(.+)/);
            if (match) {
                currentFile = match[2];
            }
        }
        // Hunk header
        if (line.startsWith('@@')) {
            if (currentHunk) {
                hunks.push(currentHunk);
            }
            const match = line.match(/@@ -(\d+),?(\d*) \+(\d+),?(\d*) @@/);
            if (match) {
                currentHunk = {
                    file: currentFile,
                    oldStart: parseInt(match[1]),
                    oldLines: parseInt(match[2] || '1'),
                    newStart: parseInt(match[3]),
                    newLines: parseInt(match[4] || '1'),
                    content: '',
                    additions: [],
                    deletions: [],
                };
            }
        }
        else if (currentHunk) {
            // Content lines
            if (line.startsWith('+') && !line.startsWith('+++')) {
                currentHunk.additions.push(line.substring(1));
                currentHunk.content += line + '\n';
            }
            else if (line.startsWith('-') && !line.startsWith('---')) {
                currentHunk.deletions.push(line.substring(1));
                currentHunk.content += line + '\n';
            }
            else if (line.startsWith(' ')) {
                currentHunk.content += line + '\n';
            }
        }
    }
    if (currentHunk) {
        hunks.push(currentHunk);
    }
    return hunks;
}
/**
 * Classify a change based on patterns
 */
function classifyChange(diff, message = '') {
    const lowerMessage = message.toLowerCase();
    const lowerDiff = diff.toLowerCase();
    // Check message patterns
    if (/\b(fix|bug|issue|error|crash|patch)\b/.test(lowerMessage))
        return 'bugfix';
    if (/\b(feat|feature|add|new|implement)\b/.test(lowerMessage))
        return 'feature';
    if (/\b(refactor|clean|improve|optimize)\b/.test(lowerMessage))
        return 'refactor';
    if (/\b(doc|readme|comment|jsdoc)\b/.test(lowerMessage))
        return 'docs';
    if (/\b(test|spec|coverage)\b/.test(lowerMessage))
        return 'test';
    if (/\b(config|ci|cd|build|deps)\b/.test(lowerMessage))
        return 'config';
    // Check diff patterns
    if (/\.(md|txt|rst)$/.test(diff))
        return 'docs';
    if (/\.(test|spec)\.[jt]sx?/.test(diff))
        return 'test';
    if (/\.(json|ya?ml|toml|ini)$/.test(diff))
        return 'config';
    // Check content patterns
    if (/\bcatch\b|\btry\b|\berror\b/.test(lowerDiff) && /\bfix\b/.test(lowerDiff))
        return 'bugfix';
    if (/\bfunction\b|\bclass\b|\bexport\b/.test(lowerDiff))
        return 'feature';
    return 'unknown';
}
/**
 * Calculate risk score for a diff
 */
function calculateRiskScore(analysis) {
    let risk = 0;
    // Size risk
    const totalChanges = analysis.totalAdditions + analysis.totalDeletions;
    if (totalChanges > 500)
        risk += 0.3;
    else if (totalChanges > 200)
        risk += 0.2;
    else if (totalChanges > 50)
        risk += 0.1;
    // Complexity risk
    if (analysis.complexity > 20)
        risk += 0.2;
    else if (analysis.complexity > 10)
        risk += 0.1;
    // File type risk
    if (analysis.file.includes('auth') || analysis.file.includes('security'))
        risk += 0.2;
    if (analysis.file.includes('database') || analysis.file.includes('migration'))
        risk += 0.15;
    if (analysis.file.includes('api') || analysis.file.includes('endpoint'))
        risk += 0.1;
    // Pattern risk (deletions of error handling, etc.)
    for (const hunk of analysis.hunks) {
        for (const del of hunk.deletions) {
            if (/\bcatch\b|\berror\b|\bvalidat/.test(del))
                risk += 0.1;
            if (/\bif\b.*\bnull\b|\bundefined\b/.test(del))
                risk += 0.05;
        }
    }
    return Math.min(1, risk);
}
/**
 * Analyze a single file diff
 */
async function analyzeFileDiff(file, diff, message = '') {
    const hunks = parseDiff(diff).filter(h => h.file === file || h.file === '');
    const totalAdditions = hunks.reduce((sum, h) => sum + h.additions.length, 0);
    const totalDeletions = hunks.reduce((sum, h) => sum + h.deletions.length, 0);
    // Calculate complexity (branch keywords in additions)
    let complexity = 0;
    for (const hunk of hunks) {
        for (const add of hunk.additions) {
            if (/\bif\b|\belse\b|\bfor\b|\bwhile\b|\bswitch\b|\bcatch\b|\?/.test(add)) {
                complexity++;
            }
        }
    }
    const category = classifyChange(diff, message);
    const analysis = {
        file,
        hunks,
        totalAdditions,
        totalDeletions,
        complexity,
        riskScore: 0,
        category,
    };
    analysis.riskScore = calculateRiskScore(analysis);
    // Generate embedding for the diff
    if ((0, onnx_embedder_1.isReady)()) {
        const diffText = hunks.map(h => h.content).join('\n');
        const result = await (0, onnx_embedder_1.embed)(`${category} change in ${file}: ${diffText.substring(0, 500)}`);
        analysis.embedding = result.embedding;
    }
    return analysis;
}
/**
 * Get diff for a commit
 */
function getCommitDiff(commitHash = 'HEAD') {
    try {
        return (0, child_process_1.execSync)(`git show ${commitHash} --format="" 2>/dev/null`, {
            encoding: 'utf8',
            maxBuffer: 10 * 1024 * 1024,
        });
    }
    catch {
        return '';
    }
}
/**
 * Get diff for staged changes
 */
function getStagedDiff() {
    try {
        return (0, child_process_1.execSync)('git diff --cached 2>/dev/null', {
            encoding: 'utf8',
            maxBuffer: 10 * 1024 * 1024,
        });
    }
    catch {
        return '';
    }
}
/**
 * Get diff for unstaged changes
 */
function getUnstagedDiff() {
    try {
        return (0, child_process_1.execSync)('git diff 2>/dev/null', {
            encoding: 'utf8',
            maxBuffer: 10 * 1024 * 1024,
        });
    }
    catch {
        return '';
    }
}
/**
 * Analyze a commit
 */
async function analyzeCommit(commitHash = 'HEAD') {
    const diff = getCommitDiff(commitHash);
    // Get commit metadata
    let message = '', author = '', date = '';
    try {
        const info = (0, child_process_1.execSync)(`git log -1 --format="%s|%an|%aI" ${commitHash} 2>/dev/null`, {
            encoding: 'utf8',
        }).trim();
        [message, author, date] = info.split('|');
    }
    catch { }
    // Parse hunks and group by file
    const hunks = parseDiff(diff);
    const fileHunks = new Map();
    for (const hunk of hunks) {
        if (!fileHunks.has(hunk.file)) {
            fileHunks.set(hunk.file, []);
        }
        fileHunks.get(hunk.file).push(hunk);
    }
    // Analyze each file
    const files = [];
    for (const [file, fileHunkList] of fileHunks) {
        const fileDiff = fileHunkList.map(h => h.content).join('\n');
        const analysis = await analyzeFileDiff(file, diff, message);
        files.push(analysis);
    }
    const totalAdditions = files.reduce((sum, f) => sum + f.totalAdditions, 0);
    const totalDeletions = files.reduce((sum, f) => sum + f.totalDeletions, 0);
    const riskScore = files.length > 0
        ? files.reduce((sum, f) => sum + f.riskScore, 0) / files.length
        : 0;
    // Generate commit embedding
    let embedding;
    if ((0, onnx_embedder_1.isReady)()) {
        const commitText = `${message}\n\nFiles changed: ${files.map(f => f.file).join(', ')}\n+${totalAdditions} -${totalDeletions}`;
        const result = await (0, onnx_embedder_1.embed)(commitText);
        embedding = result.embedding;
    }
    return {
        hash: commitHash,
        message,
        author,
        date,
        files,
        totalAdditions,
        totalDeletions,
        riskScore,
        embedding,
    };
}
/**
 * Find similar past commits based on diff embeddings
 */
async function findSimilarCommits(currentDiff, recentCommits = 50, topK = 5) {
    if (!(0, onnx_embedder_1.isReady)()) {
        await (0, onnx_embedder_1.initOnnxEmbedder)();
    }
    // Get current diff embedding
    const currentEmbedding = (await (0, onnx_embedder_1.embed)(currentDiff.substring(0, 1000))).embedding;
    // Get recent commits
    let commits = [];
    try {
        commits = (0, child_process_1.execSync)(`git log -${recentCommits} --format="%H" 2>/dev/null`, {
            encoding: 'utf8',
        }).trim().split('\n');
    }
    catch {
        return [];
    }
    // Analyze and compare
    const results = [];
    for (const hash of commits.slice(0, Math.min(commits.length, recentCommits))) {
        const analysis = await analyzeCommit(hash);
        if (analysis.embedding) {
            const similarity = cosineSimilarity(currentEmbedding, analysis.embedding);
            results.push({ hash, similarity, message: analysis.message });
        }
    }
    return results
        .sort((a, b) => b.similarity - a.similarity)
        .slice(0, topK);
}
function cosineSimilarity(a, b) {
    if (a.length !== b.length)
        return 0;
    let dotProduct = 0;
    let normA = 0;
    let normB = 0;
    for (let i = 0; i < a.length; i++) {
        dotProduct += a[i] * b[i];
        normA += a[i] * a[i];
        normB += b[i] * b[i];
    }
    const magnitude = Math.sqrt(normA) * Math.sqrt(normB);
    return magnitude === 0 ? 0 : dotProduct / magnitude;
}
exports.default = {
    parseDiff,
    classifyChange,
    calculateRiskScore,
    analyzeFileDiff,
    analyzeCommit,
    getCommitDiff,
    getStagedDiff,
    getUnstagedDiff,
    findSimilarCommits,
};
//# sourceMappingURL=diff-embeddings.js.map