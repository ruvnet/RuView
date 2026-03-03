#!/usr/bin/env ts-node
/**
 * MidStream Security Check Script
 *
 * Comprehensive security audit of MidStream components
 * Created by rUv
 */

import * as fs from 'fs';
import * as path from 'path';
import chalk from 'chalk';

// ============================================================================
// Security Check Types
// ============================================================================

interface SecurityIssue {
  severity: 'critical' | 'high' | 'medium' | 'low';
  category: string;
  file: string;
  line?: number;
  description: string;
  recommendation: string;
}

interface SecurityReport {
  timestamp: Date;
  totalIssues: number;
  critical: number;
  high: number;
  medium: number;
  low: number;
  issues: SecurityIssue[];
  passed: string[];
}

// ============================================================================
// Security Checks
// ============================================================================

class SecurityChecker {
  private issues: SecurityIssue[] = [];
  private passed: string[] = [];

  /**
   * Run all security checks
   */
  async runAllChecks(): Promise<SecurityReport> {
    console.log(chalk.bold.cyan('\n🔐 MidStream Security Check'));
    console.log(chalk.gray('═'.repeat(60)));

    await this.checkEnvironmentVariables();
    await this.checkAPIKeyExposure();
    await this.checkDependencyVulnerabilities();
    await this.checkInputValidation();
    await this.checkAuthenticationMechanisms();
    await this.checkDataEncryption();
    await this.checkRateLimiting();
    await this.checkErrorHandling();
    await this.checkLogging();
    await this.checkCORS();

    return this.generateReport();
  }

  /**
   * Check environment variables
   */
  private async checkEnvironmentVariables(): Promise<void> {
    console.log(chalk.yellow('\n📋 Checking environment variables...'));

    const envExample = path.join(__dirname, '../../.env.example');
    const env = path.join(__dirname, '../../.env');

    // Check if .env.example exists
    if (!fs.existsSync(envExample)) {
      this.issues.push({
        severity: 'medium',
        category: 'Configuration',
        file: '.env.example',
        description: '.env.example file is missing',
        recommendation: 'Create .env.example with all required environment variables',
      });
    } else {
      this.passed.push('.env.example exists');
    }

    // Check if .env is in .gitignore
    const gitignore = path.join(__dirname, '../../.gitignore');
    if (fs.existsSync(gitignore)) {
      const content = fs.readFileSync(gitignore, 'utf-8');
      if (content.includes('.env')) {
        this.passed.push('.env is in .gitignore');
      } else {
        this.issues.push({
          severity: 'high',
          category: 'Configuration',
          file: '.gitignore',
          description: '.env file not excluded from version control',
          recommendation: 'Add .env to .gitignore to prevent credential leakage',
        });
      }
    }
  }

  /**
   * Check for API key exposure
   */
  private async checkAPIKeyExposure(): Promise<void> {
    console.log(chalk.yellow('\n🔑 Checking for API key exposure...'));

    const srcDir = path.join(__dirname, '../src');
    const files = this.getAllFiles(srcDir, '.ts');

    const dangerousPatterns = [
      /['"]sk-[a-zA-Z0-9]{32,}['"]/,  // OpenAI keys
      /['"][A-Z0-9]{32,}['"]/,         // Generic API keys
      /['"]api[_-]?key['"]:\s*['"][^'"]+['"]/i,  // Hardcoded API keys
    ];

    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8');
      const lines = content.split('\n');

      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];

        for (const pattern of dangerousPatterns) {
          if (pattern.test(line) && !line.includes('process.env')) {
            this.issues.push({
              severity: 'critical',
              category: 'Credentials',
              file: path.relative(srcDir, file),
              line: i + 1,
              description: 'Potential hardcoded API key detected',
              recommendation: 'Use environment variables: process.env.API_KEY',
            });
          }
        }
      }
    }

    if (this.issues.filter((i) => i.category === 'Credentials').length === 0) {
      this.passed.push('No hardcoded API keys found');
    }
  }

  /**
   * Check dependency vulnerabilities
   */
  private async checkDependencyVulnerabilities(): Promise<void> {
    console.log(chalk.yellow('\n📦 Checking dependencies...'));

    const packageJson = path.join(__dirname, '../../package.json');
    if (fs.existsSync(packageJson)) {
      const pkg = JSON.parse(fs.readFileSync(packageJson, 'utf-8'));

      // Check for known vulnerable packages (simplified check)
      const knownVulnerable = ['event-stream@3.3.6', 'flatmap-stream'];

      const allDeps = {
        ...pkg.dependencies,
        ...pkg.devDependencies,
      };

      for (const [name, version] of Object.entries(allDeps)) {
        if (knownVulnerable.includes(name)) {
          this.issues.push({
            severity: 'high',
            category: 'Dependencies',
            file: 'package.json',
            description: `Known vulnerable package: ${name}`,
            recommendation: 'Update or remove the vulnerable package',
          });
        }
      }

      this.passed.push('Dependency check completed');
    }
  }

  /**
   * Check input validation
   */
  private async checkInputValidation(): Promise<void> {
    console.log(chalk.yellow('\n✅ Checking input validation...'));

    const srcDir = path.join(__dirname, '../src');
    const files = this.getAllFiles(srcDir, '.ts');

    let validationFound = false;

    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8');

      // Check for validation patterns
      if (
        content.includes('validate') ||
        content.includes('sanitize') ||
        content.includes('throw new Error')
      ) {
        validationFound = true;
      }

      // Check for dangerous eval/exec usage
      if (content.includes('eval(') && !content.includes('// safe')) {
        this.issues.push({
          severity: 'critical',
          category: 'Input Validation',
          file: path.relative(srcDir, file),
          description: 'Potential unsafe eval() usage',
          recommendation: 'Avoid eval(). Use safer alternatives like JSON.parse()',
        });
      }
    }

    if (validationFound) {
      this.passed.push('Input validation mechanisms found');
    }
  }

  /**
   * Check authentication mechanisms
   */
  private async checkAuthenticationMechanisms(): Promise<void> {
    console.log(chalk.yellow('\n🔐 Checking authentication...'));

    const srcDir = path.join(__dirname, '../src');
    const files = this.getAllFiles(srcDir, '.ts');

    let authFound = false;

    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8');

      if (
        content.includes('Authorization') ||
        content.includes('apiKey') ||
        content.includes('Bearer')
      ) {
        authFound = true;
      }

      // Check for insecure auth
      if (content.includes('Basic auth') && !content.includes('https')) {
        this.issues.push({
          severity: 'high',
          category: 'Authentication',
          file: path.relative(srcDir, file),
          description: 'Basic auth without HTTPS',
          recommendation: 'Always use HTTPS with Basic authentication',
        });
      }
    }

    if (authFound) {
      this.passed.push('Authentication mechanisms present');
    }
  }

  /**
   * Check data encryption
   */
  private async checkDataEncryption(): Promise<void> {
    console.log(chalk.yellow('\n🔒 Checking data encryption...'));

    const srcDir = path.join(__dirname, '../src');
    const files = this.getAllFiles(srcDir, '.ts');

    let httpsFound = false;
    let wsssFound = false;

    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8');

      // Check for HTTPS/WSS usage
      if (content.includes('https://') || content.includes('wss://')) {
        if (content.includes('https://')) httpsFound = true;
        if (content.includes('wss://')) wsssFound = true;
      }

      // Check for insecure protocols
      if (content.match(/['"]http:\/\/[^'"]+['"]/)) {
        const match = content.match(/['"]http:\/\/[^'"]+['"]/);
        if (match && !match[0].includes('localhost') && !match[0].includes('127.0.0.1')) {
          this.issues.push({
            severity: 'medium',
            category: 'Encryption',
            file: path.relative(srcDir, file),
            description: 'Insecure HTTP protocol detected',
            recommendation: 'Use HTTPS for all external connections',
          });
        }
      }
    }

    if (httpsFound) this.passed.push('HTTPS usage detected');
    if (wsssFound) this.passed.push('WSS (secure WebSocket) usage detected');
  }

  /**
   * Check rate limiting
   */
  private async checkRateLimiting(): Promise<void> {
    console.log(chalk.yellow('\n⏱️  Checking rate limiting...'));

    const srcDir = path.join(__dirname, '../src');
    const files = this.getAllFiles(srcDir, '.ts');

    let rateLimitingFound = false;

    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8');

      if (
        content.includes('rate') ||
        content.includes('throttle') ||
        content.includes('debounce') ||
        content.includes('minInterval')
      ) {
        rateLimitingFound = true;
      }
    }

    if (rateLimitingFound) {
      this.passed.push('Rate limiting mechanisms found');
    } else {
      this.issues.push({
        severity: 'low',
        category: 'Rate Limiting',
        file: 'streaming.ts',
        description: 'No rate limiting detected for API calls',
        recommendation: 'Implement rate limiting to prevent abuse',
      });
    }
  }

  /**
   * Check error handling
   */
  private async checkErrorHandling(): Promise<void> {
    console.log(chalk.yellow('\n⚠️  Checking error handling...'));

    const srcDir = path.join(__dirname, '../src');
    const files = this.getAllFiles(srcDir, '.ts');

    let errorHandlingFound = 0;

    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8');

      // Count try-catch blocks
      const tryCount = (content.match(/try\s*\{/g) || []).length;
      const catchCount = (content.match(/catch\s*\(/g) || []).length;

      if (tryCount > 0 && catchCount > 0) {
        errorHandlingFound++;
      }

      // Check for unhandled promises
      if (content.match(/\.then\(/g) && !content.match(/\.catch\(/g)) {
        this.issues.push({
          severity: 'medium',
          category: 'Error Handling',
          file: path.relative(srcDir, file),
          description: 'Promise without catch handler',
          recommendation: 'Add .catch() to handle promise rejections',
        });
      }
    }

    if (errorHandlingFound > 0) {
      this.passed.push(`Error handling found in ${errorHandlingFound} files`);
    }
  }

  /**
   * Check logging practices
   */
  private async checkLogging(): Promise<void> {
    console.log(chalk.yellow('\n📝 Checking logging practices...'));

    const srcDir = path.join(__dirname, '../src');
    const files = this.getAllFiles(srcDir, '.ts');

    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8');
      const lines = content.split('\n');

      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];

        // Check for sensitive data in logs
        if (
          line.includes('console.log') &&
          (line.includes('password') ||
            line.includes('apiKey') ||
            line.includes('secret') ||
            line.includes('token'))
        ) {
          this.issues.push({
            severity: 'high',
            category: 'Logging',
            file: path.relative(srcDir, file),
            line: i + 1,
            description: 'Potential sensitive data logging',
            recommendation: 'Never log passwords, API keys, or secrets',
          });
        }
      }
    }

    this.passed.push('Logging practices reviewed');
  }

  /**
   * Check CORS configuration
   */
  private async checkCORS(): Promise<void> {
    console.log(chalk.yellow('\n🌐 Checking CORS configuration...'));

    const srcDir = path.join(__dirname, '../src');
    const files = this.getAllFiles(srcDir, '.ts');

    let corsFound = false;

    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8');

      if (content.includes('Access-Control-Allow-Origin')) {
        corsFound = true;

        // Check for unsafe CORS
        if (content.includes('Access-Control-Allow-Origin: *')) {
          this.issues.push({
            severity: 'medium',
            category: 'CORS',
            file: path.relative(srcDir, file),
            description: 'Wildcard CORS policy detected',
            recommendation: 'Restrict CORS to specific origins in production',
          });
        }
      }
    }

    if (corsFound) {
      this.passed.push('CORS configuration present');
    }
  }

  /**
   * Get all files recursively
   */
  private getAllFiles(dir: string, ext: string): string[] {
    const files: string[] = [];

    if (!fs.existsSync(dir)) {
      return files;
    }

    const items = fs.readdirSync(dir);

    for (const item of items) {
      const fullPath = path.join(dir, item);
      const stat = fs.statSync(fullPath);

      if (stat.isDirectory()) {
        files.push(...this.getAllFiles(fullPath, ext));
      } else if (item.endsWith(ext)) {
        files.push(fullPath);
      }
    }

    return files;
  }

  /**
   * Generate security report
   */
  private generateReport(): SecurityReport {
    const critical = this.issues.filter((i) => i.severity === 'critical').length;
    const high = this.issues.filter((i) => i.severity === 'high').length;
    const medium = this.issues.filter((i) => i.severity === 'medium').length;
    const low = this.issues.filter((i) => i.severity === 'low').length;

    return {
      timestamp: new Date(),
      totalIssues: this.issues.length,
      critical,
      high,
      medium,
      low,
      issues: this.issues,
      passed: this.passed,
    };
  }
}

// ============================================================================
// Report Generation
// ============================================================================

function printReport(report: SecurityReport): void {
  console.log(chalk.bold.cyan('\n\n' + '═'.repeat(60)));
  console.log(chalk.bold.cyan('Security Report'));
  console.log(chalk.bold.cyan('═'.repeat(60)));

  console.log(chalk.gray(`Generated: ${report.timestamp.toLocaleString()}\n`));

  // Summary
  console.log(chalk.bold('Summary:'));
  console.log(`  Total Issues: ${report.totalIssues}`);
  console.log(`  ${chalk.red('Critical:')} ${report.critical}`);
  console.log(`  ${chalk.yellow('High:')} ${report.high}`);
  console.log(`  ${chalk.blue('Medium:')} ${report.medium}`);
  console.log(`  ${chalk.gray('Low:')} ${report.low}`);

  // Passed checks
  console.log(chalk.bold.green('\n✓ Passed Checks:'));
  report.passed.forEach((check) => {
    console.log(chalk.green(`  ✓ ${check}`));
  });

  // Issues
  if (report.issues.length > 0) {
    console.log(chalk.bold.red('\n✗ Issues Found:'));

    const sortedIssues = report.issues.sort((a, b) => {
      const severityOrder = { critical: 0, high: 1, medium: 2, low: 3 };
      return severityOrder[a.severity] - severityOrder[b.severity];
    });

    sortedIssues.forEach((issue, index) => {
      const severityColor =
        issue.severity === 'critical'
          ? chalk.red
          : issue.severity === 'high'
          ? chalk.yellow
          : issue.severity === 'medium'
          ? chalk.blue
          : chalk.gray;

      console.log(`\n${index + 1}. ${severityColor(`[${issue.severity.toUpperCase()}]`)} ${issue.category}`);
      console.log(`   File: ${issue.file}${issue.line ? `:${issue.line}` : ''}`);
      console.log(`   ${chalk.gray(issue.description)}`);
      console.log(`   ${chalk.green('→')} ${issue.recommendation}`);
    });
  }

  // Overall status
  console.log(chalk.bold.cyan('\n' + '═'.repeat(60)));

  if (report.critical > 0) {
    console.log(chalk.red.bold('❌ SECURITY AUDIT FAILED'));
    console.log(chalk.red(`Critical issues must be fixed before deployment`));
  } else if (report.high > 0) {
    console.log(chalk.yellow.bold('⚠️  SECURITY AUDIT WARNING'));
    console.log(chalk.yellow(`High-priority issues should be addressed`));
  } else if (report.issues.length > 0) {
    console.log(chalk.blue.bold('✓ SECURITY AUDIT PASSED'));
    console.log(chalk.blue(`Minor issues can be addressed incrementally`));
  } else {
    console.log(chalk.green.bold('✅ SECURITY AUDIT PASSED'));
    console.log(chalk.green(`No security issues detected`));
  }

  console.log(chalk.bold.cyan('═'.repeat(60) + '\n'));
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const checker = new SecurityChecker();
  const report = await checker.runAllChecks();

  printReport(report);

  // Save report
  const reportPath = path.join(__dirname, '../../security-report.json');
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  console.log(chalk.gray(`Full report saved to: ${reportPath}\n`));

  // Exit with appropriate code
  if (report.critical > 0) {
    process.exit(1);
  } else if (report.high > 0) {
    process.exit(1);
  }
}

if (require.main === module) {
  main().catch((error) => {
    console.error(chalk.red('Security check failed:'), error);
    process.exit(1);
  });
}

export { SecurityChecker, SecurityReport, SecurityIssue };
