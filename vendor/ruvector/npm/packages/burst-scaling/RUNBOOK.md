# Ruvector Burst Scaling - Operations Runbook

## Overview

This runbook provides operational procedures for managing the Ruvector adaptive burst scaling system. This system handles traffic spikes from 500M to 25B concurrent streams while maintaining <50ms p99 latency.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Normal Operations](#normal-operations)
3. [Burst Event Procedures](#burst-event-procedures)
4. [Emergency Procedures](#emergency-procedures)
5. [Monitoring & Alerts](#monitoring--alerts)
6. [Cost Management](#cost-management)
7. [Troubleshooting](#troubleshooting)
8. [Runbook Contacts](#runbook-contacts)

---

## Architecture Overview

### Components

- **Burst Predictor**: Predicts upcoming traffic spikes using event calendars and ML
- **Reactive Scaler**: Real-time auto-scaling based on metrics
- **Capacity Manager**: Global orchestration with budget controls
- **Cloud Run**: Containerized application with auto-scaling (10-1000 instances per region)
- **Global Load Balancer**: Distributes traffic across regions
- **Cloud SQL**: Database with read replicas
- **Redis**: Caching layer

### Regions

- Primary: us-central1
- Secondary: europe-west1, asia-east1
- On-demand: Additional regions can be activated

---

## Normal Operations

### Daily Checks (Automated)

✅ Verify all regions are healthy
✅ Check p99 latency < 50ms
✅ Confirm instance counts within expected range
✅ Review cost vs budget (should be ~$240k/day baseline)
✅ Check for upcoming predicted bursts

### Weekly Review

1. **Review Prediction Accuracy**
   ```bash
   npm run predictor
   ```
   Target: >85% accuracy

2. **Analyze Cost Trends**
   - Review Cloud Console billing dashboard
   - Compare actual vs predicted costs
   - Adjust budget thresholds if needed

3. **Update Event Calendar**
   - Add known upcoming events (sports, releases)
   - Review historical patterns
   - Train ML models with recent data

### Monthly Tasks

- Review and update scaling thresholds
- Audit degradation strategies
- Conduct burst simulation testing
- Update on-call documentation
- Review SLA compliance (p99 < 50ms)

---

## Burst Event Procedures

### Pre-Event (15 minutes before)

**Automatic**: Burst Predictor triggers pre-warming

**Manual Verification**:
1. Check Cloud Console for pre-warming status
2. Verify instances scaling up in predicted regions
3. Monitor cost dashboard for expected increases
4. Alert team via Slack #burst-events

### During Event

**Monitor (every 5 minutes)**:
- Dashboard: https://console.cloud.google.com/monitoring/dashboards/custom/ruvector-burst
- Key metrics:
  - Connection count (should handle 10-50x)
  - P99 latency (must stay < 50ms)
  - Error rate (must stay < 1%)
  - Instance count per region

**Scaling Actions** (if needed):
```bash
# Check current capacity
gcloud run services describe ruvector-us-central1 --region=us-central1

# Manual scale-out (emergency only)
gcloud run services update ruvector-us-central1 \
  --region=us-central1 \
  --max-instances=1500

# Check reactive scaler status
npm run scaler

# Check capacity manager
npm run manager
```

### Post-Event (within 1 hour)

1. **Verify Scale-In**
   - Instances should gradually reduce to normal levels
   - Should take 5-10 minutes after traffic normalizes

2. **Review Performance**
   - Export metrics to CSV
   - Calculate actual vs predicted load
   - Document any issues

3. **Update Patterns**
   ```bash
   # Train model with new data
   npm run predictor -- --train --event-id="world-cup-2026"
   ```

4. **Cost Analysis**
   - Compare actual cost vs budget
   - Document any overages
   - Update cost projections

---

## Emergency Procedures

### Scenario 1: Latency Spike (p99 > 100ms)

**Severity**: HIGH
**Response Time**: 2 minutes

**Actions**:
1. **Immediate**:
   ```bash
   # Force scale-out across all regions
   for region in us-central1 europe-west1 asia-east1; do
     gcloud run services update ruvector-$region \
       --region=$region \
       --min-instances=100 \
       --max-instances=1500
   done
   ```

2. **Investigate**:
   - Check Cloud SQL connections (should be < 5000)
   - Verify Redis hit rate (should be > 90%)
   - Review application logs for slow queries

3. **Escalate** if latency doesn't improve in 5 minutes

### Scenario 2: Budget Exceeded (>120% hourly limit)

**Severity**: MEDIUM
**Response Time**: 5 minutes

**Actions**:
1. **Check if legitimate burst**:
   ```bash
   npm run manager
   # Review degradation level
   ```

2. **If unexpected**:
   - Enable minor degradation:
     ```bash
     # Shed free-tier traffic
     gcloud run services update-traffic ruvector-us-central1 \
       --to-tags=premium=100
     ```

3. **If critical (>150% budget)**:
   - Enable major degradation
   - Contact finance team
   - Consider enabling hard budget limit

### Scenario 3: Region Failure

**Severity**: CRITICAL
**Response Time**: Immediate

**Actions**:
1. **Automatic**: Load balancer should route around failed region

2. **Manual Verification**:
   ```bash
   # Check backend health
   gcloud compute backend-services get-health ruvector-backend \
     --global
   ```

3. **If capacity issues**:
   ```bash
   # Scale up remaining regions
   gcloud run services update ruvector-europe-west1 \
     --region=europe-west1 \
     --max-instances=2000
   ```

4. **Activate backup region**:
   ```bash
   # Deploy to us-east1
   cd terraform
   terraform apply -var="regions=[\"us-central1\",\"europe-west1\",\"asia-east1\",\"us-east1\"]"
   ```

### Scenario 4: Database Connection Exhaustion

**Severity**: HIGH
**Response Time**: 3 minutes

**Actions**:
1. **Immediate**:
   ```bash
   # Scale up Cloud SQL
   gcloud sql instances patch ruvector-db-us-central1 \
     --cpu=32 \
     --memory=128GB

   # Increase max connections
   gcloud sql instances patch ruvector-db-us-central1 \
     --database-flags=max_connections=10000
   ```

2. **Temporary**:
   - Increase Redis cache TTL
   - Enable read-only mode for non-critical endpoints
   - Route read queries to replicas

3. **Long-term**:
   - Add more read replicas
   - Implement connection pooling
   - Review query optimization

### Scenario 5: Cascading Failures

**Severity**: CRITICAL
**Response Time**: Immediate

**Actions**:
1. **Enable Circuit Breakers**:
   - Automatic via load balancer configuration
   - Unhealthy backends ejected after 5 consecutive errors

2. **Graceful Degradation**:
   ```bash
   # Enable critical degradation mode
   npm run manager -- --degrade=critical
   ```
   - Premium tier only
   - Disable non-essential features
   - Enable maintenance page for free tier

3. **Emergency Scale-Down**:
   ```bash
   # If cascading continues, scale down to known-good state
   gcloud run services update ruvector-us-central1 \
     --region=us-central1 \
     --min-instances=50 \
     --max-instances=50
   ```

4. **Incident Response**:
   - Page on-call SRE
   - Open war room
   - Activate disaster recovery plan

---

## Monitoring & Alerts

### Cloud Monitoring Dashboard

**URL**: https://console.cloud.google.com/monitoring/dashboards/custom/ruvector-burst

**Key Metrics**:
- Total connections (all regions)
- Connections by region
- P50/P95/P99 latency
- Instance count
- CPU/Memory utilization
- Error rate
- Hourly cost
- Burst event timeline

### Alert Policies

| Alert | Threshold | Severity | Response Time |
|-------|-----------|----------|---------------|
| High P99 Latency | >50ms for 2min | HIGH | 5 min |
| Critical Latency | >100ms for 1min | CRITICAL | 2 min |
| High Error Rate | >1% for 5min | HIGH | 5 min |
| Budget Warning | >80% hourly | MEDIUM | 15 min |
| Budget Critical | >100% hourly | HIGH | 5 min |
| Region Down | 0 healthy backends | CRITICAL | Immediate |
| CPU Critical | >90% for 5min | HIGH | 5 min |
| Memory Critical | >90% for 3min | CRITICAL | 2 min |

### Notification Channels

- **Email**: ops@ruvector.io
- **PagerDuty**: Critical alerts only
- **Slack**: #alerts-burst-scaling
- **Phone**: On-call rotation (critical only)

### Log Queries

**High Latency Requests**:
```sql
resource.type="cloud_run_revision"
jsonPayload.latency > 0.1
severity >= WARNING
```

**Scaling Events**:
```sql
resource.type="cloud_run_revision"
jsonPayload.message =~ "SCALING|SCALED"
```

**Cost Events**:
```sql
jsonPayload.message =~ "BUDGET"
```

---

## Cost Management

### Budget Structure

- **Hourly**: $10,000 (~200-400 instances)
- **Daily**: $200,000 (baseline + moderate bursts)
- **Monthly**: $5,000,000 (includes major events)

### Cost Thresholds

| Level | Action | Impact |
|-------|--------|--------|
| 50% | Info log | None |
| 80% | Warning alert | None |
| 90% | Critical alert | None |
| 100% | Minor degradation | Free tier limited |
| 120% | Major degradation | Free tier shed |
| 150% | Critical degradation | Premium only |

### Cost Optimization

**Automatic**:
- Gradual scale-in after bursts
- Preemptible instances for batch jobs
- Aggressive CDN caching
- Connection pooling

**Manual**:
```bash
# Review cost by region
gcloud billing accounts list
gcloud billing projects describe ruvector-prod

# Analyze top cost drivers
gcloud alpha billing budgets list --billing-account=YOUR_ACCOUNT

# Optimize specific region
terraform apply -var="us-central1-max-instances=800"
```

### Cost Forecasting

```bash
# Generate cost forecast
npm run manager -- --forecast=7days

# Expected costs:
# - Normal week: $1.4M
# - Major event week: $2.5M
# - World Cup week: $4.8M
```

---

## Troubleshooting

### Issue: Auto-scaling not responding

**Symptoms**: Load increasing but instances not scaling

**Diagnosis**:
```bash
# Check Cloud Run auto-scaling config
gcloud run services describe ruvector-us-central1 \
  --region=us-central1 \
  --format="value(spec.template.spec.scaling)"

# Check for quota limits
gcloud compute project-info describe --project=ruvector-prod \
  | grep -A5 CPUS
```

**Resolution**:
- Verify max-instances not reached
- Check quota limits
- Review IAM permissions for service account
- Restart capacity manager

### Issue: Predictions inaccurate

**Symptoms**: Actual load differs significantly from predicted

**Diagnosis**:
```bash
npm run predictor -- --check-accuracy
```

**Resolution**:
- Update event calendar with actual times
- Retrain models with recent data
- Adjust multiplier for event types
- Review regional distribution assumptions

### Issue: Database connection pool exhausted

**Symptoms**: Connection errors, high latency

**Diagnosis**:
```bash
# Check active connections
gcloud sql operations list --instance=ruvector-db-us-central1

# Check Cloud SQL metrics
gcloud monitoring time-series list \
  --filter='metric.type="cloudsql.googleapis.com/database/postgresql/num_backends"'
```

**Resolution**:
- Scale up Cloud SQL instance
- Increase max_connections
- Add read replicas
- Review connection pooling settings

### Issue: Redis cache misses

**Symptoms**: High database load, increased latency

**Diagnosis**:
```bash
# Check Redis stats
gcloud redis instances describe ruvector-redis-us-central1 \
  --region=us-central1

# Check hit rate
gcloud monitoring time-series list \
  --filter='metric.type="redis.googleapis.com/stats/cache_hit_ratio"'
```

**Resolution**:
- Increase Redis memory
- Review cache TTL settings
- Implement cache warming for predicted bursts
- Review cache key patterns

---

## Runbook Contacts

### On-Call Rotation

**Primary On-Call**: Check PagerDuty
**Secondary On-Call**: Check PagerDuty
**Escalation**: VP Engineering

### Team Contacts

| Role | Contact | Phone |
|------|---------|-------|
| SRE Lead | sre-lead@ruvector.io | +1-XXX-XXX-XXXX |
| DevOps | devops@ruvector.io | +1-XXX-XXX-XXXX |
| Engineering Manager | eng-mgr@ruvector.io | +1-XXX-XXX-XXXX |
| VP Engineering | vp-eng@ruvector.io | +1-XXX-XXX-XXXX |

### External Contacts

| Service | Contact | SLA |
|---------|---------|-----|
| GCP Support | Premium Support | 15 min |
| PagerDuty | support@pagerduty.com | 1 hour |
| Network Provider | NOC hotline | 30 min |

### War Room

**Zoom**: https://zoom.us/j/ruvector-war-room
**Slack**: #incident-response
**Docs**: https://docs.ruvector.io/incidents

---

## Appendix

### Quick Reference Commands

```bash
# Check system status
npm run manager

# View current metrics
gcloud monitoring dashboards list

# Force scale-out
gcloud run services update ruvector-REGION --max-instances=1500

# Enable degradation
npm run manager -- --degrade=minor

# Check predictions
npm run predictor

# View logs
gcloud logging read "resource.type=cloud_run_revision" --limit=50

# Check costs
gcloud billing accounts list
```

### Terraform Quick Reference

```bash
# Initialize
cd terraform && terraform init

# Plan changes
terraform plan -var-file="prod.tfvars"

# Apply changes
terraform apply -var-file="prod.tfvars"

# Emergency scale-out
terraform apply -var="max_instances=2000"

# Add region
terraform apply -var='regions=["us-central1","europe-west1","asia-east1","us-east1"]'
```

### Health Check URLs

- **Application**: https://api.ruvector.io/health
- **Database**: https://api.ruvector.io/health/db
- **Redis**: https://api.ruvector.io/health/redis
- **Load Balancer**: Check Cloud Console

### Disaster Recovery

**RTO (Recovery Time Objective)**: 15 minutes
**RPO (Recovery Point Objective)**: 5 minutes

**Backup Locations**:
- Database: Point-in-time recovery (7 days)
- Configuration: Git repository
- Terraform state: GCS bucket (versioned)

**Recovery Procedure**:
1. Restore from latest backup
2. Deploy infrastructure via Terraform
3. Validate health checks
4. Update DNS if needed
5. Resume traffic

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-01-20 | DevOps Team | Initial version |

---

**Last Updated**: 2025-01-20
**Next Review**: 2025-02-20
**Owner**: SRE Team
