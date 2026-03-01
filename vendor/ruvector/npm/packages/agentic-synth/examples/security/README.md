# Security Testing Examples

⚠️ **ETHICAL USE AND RESPONSIBLE DISCLOSURE ONLY** ⚠️

## Critical Warning

These examples are provided **EXCLUSIVELY** for:

- ✅ Authorized penetration testing with written permission
- ✅ Defensive security research in controlled environments
- ✅ Security awareness training programs
- ✅ Development and testing of security tools
- ✅ Academic research with proper ethical approval
- ✅ Red team exercises within your own organization
- ✅ Compliance testing for regulatory requirements

**NEVER** use these examples for:

- ❌ Unauthorized access to systems or networks
- ❌ Attacking systems without explicit written permission
- ❌ Malicious activities of any kind
- ❌ Testing third-party systems without authorization
- ❌ Violating computer fraud and abuse laws
- ❌ Bypassing security controls on production systems
- ❌ Any illegal or unethical activities

## Legal Disclaimer

**YOU ARE SOLELY RESPONSIBLE FOR YOUR ACTIONS**

Using these tools and examples against systems you don't own or don't have explicit authorization to test is **ILLEGAL** in most jurisdictions and may violate:

- Computer Fraud and Abuse Act (CFAA) - USA
- Computer Misuse Act - UK
- European Convention on Cybercrime
- Local and international cybercrime laws

Unauthorized access can result in:
- Criminal prosecution
- Civil liability
- Significant financial penalties
- Imprisonment

**ALWAYS obtain written authorization before conducting security testing.**

## Overview

This directory contains synthetic security testing data generators using `agentic-synth`. These tools help security professionals generate realistic test data for defensive security operations, tool development, and training.

## Files

### 1. `vulnerability-testing.ts`

Generates test data for common web application vulnerabilities:

- **SQL Injection Payloads** - Test input validation and parameterized queries
- **XSS Attack Vectors** - Validate output encoding and CSP
- **CSRF Test Scenarios** - Test token validation and SameSite cookies
- **Authentication Bypass Tests** - Validate authentication mechanisms
- **API Abuse Patterns** - Test rate limiting and API security controls
- **OWASP Top 10 Tests** - Comprehensive vulnerability testing suite

**Use Cases:**
- Web application security scanner development
- WAF (Web Application Firewall) rule testing
- Security code review training
- DevSecOps pipeline validation

### 2. `threat-simulation.ts`

Generates threat actor behavior simulations:

- **Brute Force Attack Patterns** - Test account lockout mechanisms
- **DDoS Traffic Simulation** - Validate DDoS mitigation
- **Malware Behavior Patterns** - Test EDR/XDR systems
- **Phishing Campaign Data** - Security awareness training
- **Insider Threat Scenarios** - UBA system validation
- **Zero-Day Exploit Indicators** - Threat intelligence testing

**Use Cases:**
- SOC analyst training
- Incident response preparedness
- Threat detection rule development
- Security monitoring system validation

### 3. `security-audit.ts`

Generates security audit and compliance data:

- **User Access Patterns** - Detect privilege escalation
- **Permission Change Audits** - Track access control modifications
- **Configuration Change Monitoring** - Security-sensitive config tracking
- **Compliance Violation Scenarios** - GDPR, HIPAA, PCI-DSS testing
- **Security Event Correlations** - SIEM correlation rule testing
- **DLP Audit Data** - Data loss prevention policy validation

**Use Cases:**
- SIEM and log analysis tool development
- Compliance reporting automation
- Security audit automation
- Insider threat detection system testing

### 4. `penetration-testing.ts`

Generates penetration testing datasets:

- **Network Scanning Results** - Vulnerability scanner testing
- **Port Enumeration Data** - Service identification validation
- **Service Fingerprinting** - Version detection testing
- **Exploitation Attempt Logs** - Exploit detection system validation
- **Post-Exploitation Activity** - Lateral movement detection
- **Pentest Report Data** - Reporting system development

**Use Cases:**
- Penetration testing tool development
- Red team exercise planning
- Security assessment automation
- Vulnerability management system testing

## Installation

```bash
# From the monorepo root
npm install

# Or specifically for agentic-synth
cd packages/agentic-synth
npm install
```

## Configuration

Set up your Anthropic API key:

```bash
export ANTHROPIC_API_KEY='your-api-key-here'
```

Or create a `.env` file in the agentic-synth directory:

```
ANTHROPIC_API_KEY=your-api-key-here
```

## Usage Examples

### Basic Usage

```typescript
import {
  generateSQLInjectionPayloads,
  generateXSSVectors
} from './security/vulnerability-testing';

// Generate SQL injection test payloads
const sqlPayloads = await generateSQLInjectionPayloads();
console.log(sqlPayloads);

// Generate XSS test vectors
const xssVectors = await generateXSSVectors();
console.log(xssVectors);
```

### Running Complete Test Suites

```typescript
import { runVulnerabilityTests } from './security/vulnerability-testing';
import { runThreatSimulations } from './security/threat-simulation';
import { runSecurityAudits } from './security/security-audit';
import { runPenetrationTests } from './security/penetration-testing';

// Run all vulnerability tests
const vulnResults = await runVulnerabilityTests();

// Run all threat simulations
const threatResults = await runThreatSimulations();

// Run all security audits
const auditResults = await runSecurityAudits();

// Run all penetration tests
const pentestResults = await runPenetrationTests();
```

### Customizing Generation

```typescript
import { AgenticSynth } from 'agentic-synth';

const synth = new AgenticSynth({
  temperature: 0.8,  // Higher for more variety
  maxRetries: 3
});

const customData = await synth.generate({
  prompt: 'Generate custom security test data...',
  schema: {
    // Your custom JSON schema
  }
});
```

## Best Practices

### 1. Authorization First

**Before any security testing:**

```
┌─────────────────────────────────────────────┐
│  OBTAIN WRITTEN AUTHORIZATION               │
│                                             │
│  ✓ Scope definition                        │
│  ✓ Time windows                            │
│  ✓ Acceptable techniques                   │
│  ✓ Emergency contacts                      │
│  ✓ Legal review                            │
└─────────────────────────────────────────────┘
```

### 2. Controlled Environments

- Use isolated test networks
- Deploy honeypots for realistic testing
- Separate production from testing
- Implement proper segmentation
- Monitor all testing activities

### 3. Responsible Disclosure

If you discover real vulnerabilities:

1. **Do not exploit** beyond proof of concept
2. **Document** findings professionally
3. **Report** to appropriate parties immediately
4. **Follow** responsible disclosure timelines
5. **Respect** confidentiality agreements

### 4. Data Handling

- Never use real user data in tests
- Sanitize all test data before sharing
- Encrypt sensitive test artifacts
- Properly dispose of test data
- Follow data protection regulations

### 5. Tool Safety

```typescript
// Always validate before execution
if (!hasAuthorization()) {
  throw new Error('Unauthorized testing attempt blocked');
}

// Log all activities
logSecurityTestActivity({
  action: 'vulnerability_scan',
  target: authorizedTarget,
  timestamp: new Date(),
  authorization: authorizationId
});

// Implement rate limiting
const rateLimiter = new RateLimiter({
  maxRequestsPerMinute: 10
});
```

## Educational Use

These examples are valuable for:

### Security Training

- Hands-on labs for security courses
- Certification preparation (CEH, OSCP, etc.)
- Capture the Flag (CTF) competitions
- Security awareness programs

### Tool Development

- Building security testing frameworks
- Creating custom vulnerability scanners
- Developing SIEM correlation rules
- Implementing IDS/IPS signatures

### Research

- Security research projects
- Academic publications
- Threat modeling exercises
- Risk assessment frameworks

## Security Testing Workflow

```
1. AUTHORIZATION
   ↓
2. RECONNAISSANCE (Passive)
   ↓
3. SCANNING (Active, if authorized)
   ↓
4. ENUMERATION
   ↓
5. EXPLOITATION (Controlled)
   ↓
6. POST-EXPLOITATION (Limited)
   ↓
7. DOCUMENTATION
   ↓
8. REPORTING
   ↓
9. REMEDIATION SUPPORT
   ↓
10. RE-TESTING
```

## Ethical Guidelines

### DO:

✅ Get explicit written permission
✅ Stay within defined scope
✅ Report vulnerabilities responsibly
✅ Protect client confidentiality
✅ Document all activities
✅ Follow industry standards (OWASP, NIST, etc.)
✅ Maintain professional ethics
✅ Provide remediation guidance
✅ Respect privacy and data protection laws

### DON'T:

❌ Test without authorization
❌ Exceed defined scope
❌ Cause damage or disruption
❌ Access or exfiltrate real data
❌ Share findings publicly without permission
❌ Use discoveries for personal gain
❌ Ignore responsible disclosure procedures
❌ Test in production without approval
❌ Bypass security controls unnecessarily

## Compliance Considerations

When generating test data for compliance testing:

### GDPR (General Data Protection Regulation)

- Use synthetic data only
- Don't process real personal data
- Document data processing activities
- Implement data minimization
- Ensure right to erasure

### HIPAA (Health Insurance Portability and Accountability Act)

- Never use real PHI (Protected Health Information)
- Test with synthetic medical data only
- Ensure encryption at rest and in transit
- Document all security testing activities
- Maintain audit logs

### PCI-DSS (Payment Card Industry Data Security Standard)

- Never test with real cardholder data
- Use test card numbers only
- Implement network segmentation
- Conduct quarterly vulnerability scans
- Perform annual penetration tests

## Support and Resources

### Official Resources

- [OWASP Testing Guide](https://owasp.org/www-project-web-security-testing-guide/)
- [NIST Cybersecurity Framework](https://www.nist.gov/cyberframework)
- [MITRE ATT&CK Framework](https://attack.mitre.org/)
- [CVE Database](https://cve.mitre.org/)
- [NVD (National Vulnerability Database)](https://nvd.nist.gov/)

### Community

- OWASP Local Chapters
- DEF CON Groups
- SANS Internet Storm Center
- Bugcrowd and HackerOne forums

### Training

- [SANS Security Training](https://www.sans.org/)
- [Offensive Security Certifications](https://www.offensive-security.com/)
- [eLearnSecurity Courses](https://elearnsecurity.com/)
- [Cybrary Free Courses](https://www.cybrary.it/)

## Contributing

When contributing security testing examples:

1. **Ensure ethical use** - All examples must be defensive
2. **Include warnings** - Clear ethical use statements
3. **Document thoroughly** - Explain intended use cases
4. **Test safely** - Validate in isolated environments
5. **Review carefully** - Security team approval required

## License

These examples are provided for educational and authorized testing purposes only. Users are solely responsible for ensuring compliance with all applicable laws and regulations.

---

## Final Reminder

🚨 **CRITICAL** 🚨

**UNAUTHORIZED COMPUTER ACCESS IS A CRIME**

These tools are powerful and must be used responsibly. The line between ethical hacking and criminal activity is **authorization**. Always obtain explicit written permission before conducting any security testing.

**When in doubt, don't test. Ask first.**

---

*Generated using agentic-synth - Synthetic data for ethical security testing*

**Remember: With great power comes great responsibility. Use these tools wisely.**
