// SPDX-License-Identifier: MIT

import { readFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { canonicalJson, sha256 } from '../../src/canonical.js';
import { isPlainObject, normalizedIso } from '../../src/validation.js';

const MAX_PLAN_BYTES = 10 * 1024 * 1024;
const MAX_REVIEW_WINDOW_MS = 24 * 60 * 60 * 1000;
const GOOGLE_PROVIDER_FULL_NAME = 'registry.terraform.io/hashicorp/google';
const GOOGLE_PROVIDER_NAME = 'google';
const DIGEST_RE = /^sha256:[0-9a-f]{64}$/u;
const PROJECT_RE = /^[a-z][a-z0-9-]{4,28}[a-z0-9]$/u;
const REGION_RE = /^[a-z]+(?:-[a-z0-9]+)+[0-9]$/u;
const SERVICE_NAME_RE = /^[a-z](?:[a-z0-9-]{0,47}[a-z0-9])?$/u;
const ARTIFACT_REGISTRY_DIGEST = /^([a-z0-9-]+)-docker\.pkg\.dev\/([a-z][a-z0-9-]{4,28}[a-z0-9])\/[a-z0-9._-]+\/[a-z0-9._/-]+@sha256:[0-9a-f]{64}$/u;
const PUBLIC_MEMBERS = new Set(['allAuthenticatedUsers', 'allUsers']);
const REQUIRED_SERVICES = Object.freeze([
  'artifactregistry.googleapis.com',
  'cloudresourcemanager.googleapis.com',
  'iam.googleapis.com',
  'run.googleapis.com',
  'serviceusage.googleapis.com',
]);
const BASE_RESOURCE_TYPES = Object.freeze({
  'google_cloud_run_v2_service.control_plane': 'google_cloud_run_v2_service',
  'google_service_account.control_plane': 'google_service_account',
});
const HEARTBEAT_RESOURCE_TYPES = Object.freeze({
  'google_cloud_run_v2_service_iam_member.heartbeat_invoker[0]': 'google_cloud_run_v2_service_iam_member',
  'google_cloud_scheduler_job.heartbeat[0]': 'google_cloud_scheduler_job',
  'google_project_service.scheduler[0]': 'google_project_service',
  'google_service_account.heartbeat[0]': 'google_service_account',
});
const ALLOWED_CHANGE_ACTIONS = new Set(['["create"]', '["no-op"]', '["update"]']);
const REVIEW_FIELDS = new Set([
  'approvedImage',
  'approvedImageEvidence',
  'approvedImageEvidenceDigest',
  'expiresAt',
  'heartbeatEnabled',
  'heartbeatServiceUri',
  'planDigest',
  'projectId',
  'projectNumber',
  'region',
  'reviewedAt',
  'schema',
  'serviceName',
]);
const PLANNED_RESOURCE_FIELDS = new Set(['address', 'index', 'mode', 'name', 'provider_name', 'schema_version', 'sensitive_values', 'type', 'values']);
const CHANGE_RESOURCE_FIELDS = new Set(['address', 'change', 'deposed', 'index', 'mode', 'module_address', 'name', 'provider_name', 'type']);
const CHANGE_FIELDS = new Set(['actions', 'after', 'after_identity', 'after_sensitive', 'after_unknown', 'before', 'before_identity', 'before_sensitive', 'generated_config', 'importing', 'replace_paths']);
const PROJECT_SERVICE_FIELDS = new Set(['check_if_service_has_usage_on_destroy', 'disable_dependent_services', 'disable_on_destroy', 'id', 'project', 'service', 'timeouts']);
const SERVICE_ACCOUNT_FIELDS = new Set(['account_id', 'description', 'disabled', 'display_name', 'email', 'id', 'member', 'name', 'project', 'timeouts', 'unique_id']);
const CLOUD_RUN_FIELDS = new Set(['annotations', 'binary_authorization', 'build_config', 'client', 'client_version', 'conditions', 'custom_audiences', 'deletion_protection', 'description', 'effective_annotations', 'effective_labels', 'etag', 'generation', 'iap_enabled', 'id', 'ingress', 'invoker_iam_disabled', 'labels', 'launch_stage', 'location', 'name', 'project', 'reconciling', 'template', 'terminal_condition', 'terraform_labels', 'traffic', 'traffic_statuses', 'uri']);
const TEMPLATE_FIELDS = new Set(['annotations', 'containers', 'encryption_key', 'execution_environment', 'gpu_zonal_redundancy_disabled', 'labels', 'max_instance_request_concurrency', 'node_selector', 'revision', 'scaling', 'service_account', 'session_affinity', 'timeout', 'volumes', 'vpc_access']);
const SCALING_FIELDS = new Set(['max_instance_count', 'min_instance_count']);
const CONTAINER_FIELDS = new Set(['args', 'base_image_uri', 'build_info', 'command', 'depends_on', 'env', 'image', 'name', 'ports', 'resources', 'startup_probe', 'volume_mounts', 'working_dir']);
const CONTAINER_RESOURCE_FIELDS = new Set(['cpu_idle', 'limits', 'startup_cpu_boost']);
const PROBE_FIELDS = new Set(['failure_threshold', 'grpc', 'http_get', 'initial_delay_seconds', 'period_seconds', 'tcp_socket', 'timeout_seconds']);
const HTTP_GET_FIELDS = new Set(['http_headers', 'path', 'port']);
const IAM_MEMBER_FIELDS = new Set(['condition', 'etag', 'id', 'location', 'member', 'name', 'project', 'role']);
const SCHEDULER_FIELDS = new Set(['attempt_deadline', 'description', 'http_target', 'id', 'name', 'paused', 'project', 'region', 'retry_config', 'schedule', 'state', 'time_zone', 'timeouts']);
const HTTP_TARGET_FIELDS = new Set(['body', 'headers', 'http_method', 'oauth_token', 'oidc_token', 'uri']);
const OIDC_FIELDS = new Set(['audience', 'service_account_email']);
const IMAGE_EVIDENCE_FIELDS = new Set(['image', 'provenanceDigest', 'recordedAt', 'sbomDigest', 'schema', 'sourceRevisionDigest', 'vulnerabilityScanDigest']);
const PROVIDER_CONFIG_FIELDS = new Set(['expressions', 'full_name', 'name']);
const CONFIG_RESOURCE_FIELDS = new Set(['address', 'count_expression', 'expressions', 'for_each_expression', 'mode', 'name', 'provider_config_key', 'provisioners', 'schema_version', 'type']);
const PLAN_VARIABLE_NAMES = new Set(['container_image', 'enable_heartbeat', 'image_evidence_digest', 'maximum_instances', 'minimum_instances', 'project_id', 'region', 'service_name']);
const CONFIGURED_RESOURCE_TYPES = new Map([
  ['google_cloud_run_v2_service.control_plane', 'google_cloud_run_v2_service'],
  ['google_cloud_run_v2_service_iam_member.heartbeat_invoker', 'google_cloud_run_v2_service_iam_member'],
  ['google_cloud_scheduler_job.heartbeat', 'google_cloud_scheduler_job'],
  ['google_project_service.required', 'google_project_service'],
  ['google_project_service.scheduler', 'google_project_service'],
  ['google_service_account.control_plane', 'google_service_account'],
  ['google_service_account.heartbeat', 'google_service_account'],
]);

function expectedRuntimeEmail(projectId) {
  return `ruv-social-control@${projectId}.iam.gserviceaccount.com`;
}

function expectedHeartbeatEmail(projectId) {
  return `ruv-social-heartbeat@${projectId}.iam.gserviceaccount.com`;
}

function expectedResources(heartbeatEnabled) {
  const entries = Object.entries(BASE_RESOURCE_TYPES);
  for (const service of REQUIRED_SERVICES) {
    entries.push([`google_project_service.required["${service}"]`, 'google_project_service']);
  }
  if (heartbeatEnabled) entries.push(...Object.entries(HEARTBEAT_RESOURCE_TYPES));
  return new Map(entries);
}

function collectResources(module, output = []) {
  if (!isPlainObject(module)) return output;
  if (Array.isArray(module.resources)) output.push(...module.resources);
  for (const child of module.child_modules || []) collectResources(child, output);
  return output;
}

function validateConfiguration(plan, violations) {
  const configuration = plan.configuration;
  if (!isPlainObject(configuration)) {
    violations.push('Terraform plan configuration must be present.');
    return;
  }
  const providerConfig = configuration.provider_config;
  if (!isPlainObject(providerConfig)) {
    violations.push('Terraform configuration must contain exactly one google provider configuration.');
  } else {
    if (Object.keys(providerConfig).length !== 1 || !isPlainObject(providerConfig.google)) violations.push('Terraform configuration must contain exactly one google provider configuration.');
    if (isPlainObject(providerConfig.google)) {
      const google = providerConfig.google;
      rejectUnknownFields(google, PROVIDER_CONFIG_FIELDS, 'Terraform google provider configuration', violations);
      requireEqual(google.name, 'google', 'Terraform provider name', violations);
      requireEqual(google.full_name, GOOGLE_PROVIDER_FULL_NAME, 'Terraform provider full name', violations);
      requireEmpty(google.expressions, 'Terraform provider expressions', violations);
    }
  }

  const root = configuration.root_module;
  if (!isPlainObject(root) || !Array.isArray(root.resources)) {
    violations.push('Terraform configuration root resources must be present.');
    return;
  }
  requireEmpty(root.module_calls, 'Terraform module calls', violations);
  const indexed = new Map();
  for (const resource of root.resources) {
    const address = resource?.address;
    if (typeof address !== 'string') {
      violations.push('Terraform configuration resource address is missing.');
      continue;
    }
    if (indexed.has(address)) violations.push(`Terraform configuration duplicate address ${address}.`);
    indexed.set(address, resource);
    rejectUnknownFields(resource, CONFIG_RESOURCE_FIELDS, `Terraform configuration ${address}`, violations);
    const expectedType = CONFIGURED_RESOURCE_TYPES.get(address);
    if (!expectedType) violations.push(`${address}: configuration resource is outside the exact Phase G1 source graph.`);
    if (resource?.mode !== 'managed') violations.push(`${address}: configuration resource mode must be managed.`);
    if (expectedType && resource?.type !== expectedType) violations.push(`${address}: configuration resource type must be exactly ${expectedType}.`);
    requireEqual(resource?.provider_config_key, 'google', `${address} provider configuration`, violations);
    requireEmpty(resource?.provisioners, `${address} provisioners`, violations);
  }
  for (const address of CONFIGURED_RESOURCE_TYPES.keys()) if (!indexed.has(address)) violations.push(`Terraform configuration resource ${address} is missing.`);
  if (root.resources.length !== CONFIGURED_RESOURCE_TYPES.size) violations.push(`Terraform configuration resource count must be exactly ${CONFIGURED_RESOURCE_TYPES.size}.`);
}

function validatePlanVariables(plan, review, violations) {
  const variables = plan.variables;
  if (!isPlainObject(variables)) {
    violations.push('Terraform plan variables must be present.');
    return null;
  }
  for (const key of Object.keys(variables)) if (!PLAN_VARIABLE_NAMES.has(key)) violations.push(`Terraform plan variable ${key} is not allowed.`);
  for (const key of PLAN_VARIABLE_NAMES) if (!Object.hasOwn(variables, key)) violations.push(`Terraform plan variable ${key} is missing.`);
  if (Object.keys(variables).length !== PLAN_VARIABLE_NAMES.size) violations.push(`Terraform plan variable count must be exactly ${PLAN_VARIABLE_NAMES.size}.`);

  const value = (name) => {
    const record = variables[name];
    if (!isPlainObject(record) || Object.keys(record).length !== 1 || !Object.hasOwn(record, 'value')) {
      violations.push(`Terraform plan variable ${name} must contain only value.`);
      return undefined;
    }
    return record.value;
  };
  requireEqual(value('project_id'), review.projectId, 'Terraform project_id variable', violations);
  requireEqual(value('region'), review.region, 'Terraform region variable', violations);
  requireEqual(value('service_name'), review.serviceName, 'Terraform service_name variable', violations);
  requireEqual(value('container_image'), review.approvedImage, 'Terraform container_image variable', violations);
  requireEqual(value('image_evidence_digest'), review.approvedImageEvidenceDigest, 'Terraform image_evidence_digest variable', violations);
  requireEqual(value('enable_heartbeat'), review.heartbeatEnabled, 'Terraform enable_heartbeat variable', violations);
  const minimum = value('minimum_instances');
  const maximum = value('maximum_instances');
  if (!Number.isInteger(minimum) || minimum < 0 || minimum > 3) violations.push('Terraform minimum_instances variable is invalid.');
  if (!Number.isInteger(maximum) || maximum < 1 || maximum > 3) violations.push('Terraform maximum_instances variable is invalid.');
  if (Number.isInteger(minimum) && Number.isInteger(maximum) && minimum > maximum) violations.push('Terraform variable minimum exceeds maximum.');
  return { maximum, minimum };
}

function hasTrue(value) {
  if (value === true) return true;
  if (Array.isArray(value)) return value.some(hasTrue);
  if (isPlainObject(value)) return Object.values(value).some(hasTrue);
  return false;
}

function rejectUnknownFields(value, allowed, label, violations) {
  if (!isPlainObject(value)) {
    violations.push(`${label} must be an object.`);
    return;
  }
  for (const key of Object.keys(value)) if (!allowed.has(key)) violations.push(`${label}.${key} is not allowed.`);
}

function requireEmpty(value, label, violations) {
  if (!emptyCollection(value)) violations.push(`${label} must be absent or empty.`);
}

function validateImageEvidence(review, now, violations) {
  const evidence = review.approvedImageEvidence;
  rejectUnknownFields(evidence, IMAGE_EVIDENCE_FIELDS, 'approvedImageEvidence', violations);
  if (!isPlainObject(evidence)) return;
  for (const key of IMAGE_EVIDENCE_FIELDS) if (!Object.hasOwn(evidence, key)) violations.push(`approvedImageEvidence.${key} is required.`);
  requireEqual(evidence.schema, 'SocialImageEvidenceV1', 'approvedImageEvidence schema', violations);
  requireEqual(evidence.image, review.approvedImage, 'approvedImageEvidence image', violations);
  for (const key of ['provenanceDigest', 'sbomDigest', 'sourceRevisionDigest', 'vulnerabilityScanDigest']) {
    if (!DIGEST_RE.test(evidence[key] || '')) violations.push(`approvedImageEvidence.${key} must be sha256.`);
  }
  if (!normalizedIso(evidence.recordedAt) || Date.parse(evidence.recordedAt) > now.getTime()) violations.push('approvedImageEvidence.recordedAt must be normalized and not future dated.');
  if (DIGEST_RE.test(review.approvedImageEvidenceDigest || '') && sha256(evidence) !== review.approvedImageEvidenceDigest) violations.push('Reviewed image evidence digest does not match the canonical evidence record.');
}

function exactSingleton(value, label, violations) {
  if (!Array.isArray(value) || value.length !== 1 || !isPlainObject(value[0])) {
    violations.push(`${label} must contain exactly one block.`);
    return null;
  }
  return value[0];
}

function emptyCollection(value) {
  if (value === undefined || value === null || value === '') return true;
  if (Array.isArray(value)) return value.length === 0;
  if (isPlainObject(value)) return Object.keys(value).length === 0;
  return false;
}

function validateReview(plan, review, now, violations) {
  if (!isPlainObject(review)) {
    violations.push('A SocialGcpPlanReviewV1 input is required.');
    return null;
  }
  const unknown = Object.keys(review).filter((key) => !REVIEW_FIELDS.has(key));
  const missing = [...REVIEW_FIELDS].filter((key) => !Object.hasOwn(review, key));
  if (unknown.length > 0 || missing.length > 0) violations.push('Plan review fields are incomplete or unknown.');
  if (review.schema !== 'SocialGcpPlanReviewV1') violations.push('Plan review schema mismatch.');
  if (!DIGEST_RE.test(review.planDigest || '')) violations.push('Reviewed plan digest is invalid.');
  if (DIGEST_RE.test(review.planDigest || '') && review.planDigest !== sha256(plan)) violations.push('Reviewed plan digest does not match the canonical Terraform plan.');
  if (!PROJECT_RE.test(review.projectId || '')) violations.push('Reviewed project ID is invalid.');
  if (!/^[0-9]{12}$/u.test(review.projectNumber || '')) violations.push('Reviewed project number must contain exactly 12 digits.');
  if (!REGION_RE.test(review.region || '')) violations.push('Reviewed region is invalid.');
  if (!SERVICE_NAME_RE.test(review.serviceName || '')) violations.push('Reviewed service name is invalid.');
  if (typeof review.heartbeatEnabled !== 'boolean') violations.push('Reviewed heartbeat state must be boolean.');
  const imageMatch = ARTIFACT_REGISTRY_DIGEST.exec(review.approvedImage || '');
  if (!imageMatch) {
    violations.push('Reviewed image is not a full Artifact Registry digest URI.');
  } else {
    requireEqual(imageMatch[1], review.region, 'Reviewed image registry location', violations);
    requireEqual(imageMatch[2], review.projectId, 'Reviewed image project', violations);
  }
  if (!DIGEST_RE.test(review.approvedImageEvidenceDigest || '')) violations.push('Reviewed image evidence digest is invalid.');
  validateImageEvidence(review, now, violations);
  if (review.heartbeatEnabled === false && review.heartbeatServiceUri !== null) violations.push('Reviewed heartbeat service URI must be null when heartbeat is disabled.');
  if (review.heartbeatEnabled === true) validatedServiceUri(review.heartbeatServiceUri, 'Reviewed heartbeat service URI', violations, review.serviceName, review.region, review.projectNumber);

  const reviewedAtValid = normalizedIso(review.reviewedAt);
  const expiresAtValid = normalizedIso(review.expiresAt);
  if (!reviewedAtValid || !expiresAtValid) violations.push('Plan review timestamps must be normalized ISO.');
  if (reviewedAtValid && Date.parse(review.reviewedAt) > now.getTime()) violations.push('Plan review time is in the future.');
  if (expiresAtValid && Date.parse(review.expiresAt) <= now.getTime()) violations.push('Plan review is expired.');
  if (reviewedAtValid && expiresAtValid) {
    const duration = Date.parse(review.expiresAt) - Date.parse(review.reviewedAt);
    if (duration <= 0 || duration > MAX_REVIEW_WINDOW_MS) violations.push('Plan review validity window must be greater than zero and no more than 24 hours.');
  }
  return review;
}

function indexExactResources(resources, expected, label, violations, { requireValues = true } = {}) {
  const indexed = new Map();
  for (const resource of resources) {
    const address = resource?.address;
    if (typeof address !== 'string') {
      violations.push(`${label}: resource address is missing.`);
      continue;
    }
    if (indexed.has(address)) violations.push(`${label}: duplicate resource address ${address}.`);
    indexed.set(address, resource);
    rejectUnknownFields(resource, requireValues ? PLANNED_RESOURCE_FIELDS : CHANGE_RESOURCE_FIELDS, `${label}.${address}`, violations);
    const expectedType = expected.get(address);
    if (!expectedType) violations.push(`${address}: resource is outside the exact Phase G1 graph.`);
    if (resource?.mode !== 'managed') violations.push(`${address}: resource mode must be managed.`);
    if (expectedType && resource?.type !== expectedType) violations.push(`${address}: resource type must be exactly ${expectedType}.`);
    requireEqual(resource?.provider_name, GOOGLE_PROVIDER_NAME, `${label}.${address} provider`, violations);
    if (requireValues && !isPlainObject(resource?.values)) violations.push(`${address}: resource values are missing or unresolved.`);
    if (requireValues) {
      if (!isPlainObject(resource?.sensitive_values)) violations.push(`${address}: planned sensitive_values must be a plain object.`);
      else if (hasTrue(resource.sensitive_values)) violations.push(`${address}: planned sensitive values are prohibited.`);
    }
  }
  for (const address of expected.keys()) {
    if (!indexed.has(address)) violations.push(`${label}: required resource ${address} is missing.`);
  }
  if (resources.length !== expected.size) violations.push(`${label}: resource count must be exactly ${expected.size}.`);
  return indexed;
}

function validateResourceChanges(plan, planned, expected, violations) {
  if (!Array.isArray(plan.resource_changes)) {
    violations.push('Terraform resource_changes must be present for exact plan review.');
    return;
  }
  const changes = indexExactResources(plan.resource_changes, expected, 'resource_changes', violations, { requireValues: false });
  for (const address of expected.keys()) {
    const plannedResource = planned.get(address);
    const change = changes.get(address);
    if (!plannedResource || !change) continue;
    if (!isPlainObject(change.change)) {
      violations.push(`${address}: change record is missing.`);
      continue;
    }
    rejectUnknownFields(change.change, CHANGE_FIELDS, `${address}.change`, violations);
    requireEmpty(change.deposed, `${address} deposed instance`, violations);
    requireEmpty(change.change.importing, `${address} importing`, violations);
    requireEmpty(change.change.generated_config, `${address} generated configuration`, violations);
    requireEmpty(change.change.replace_paths, `${address} replacement paths`, violations);
    requireEmpty(change.change.before_identity, `${address} before identity`, violations);
    requireEmpty(change.change.after_identity, `${address} after identity`, violations);
    if (!ALLOWED_CHANGE_ACTIONS.has(JSON.stringify(change.change.actions))) violations.push(`${address}: create, update, or no-op is the only allowed action.`);
    if (!isPlainObject(change.change.after)) {
      violations.push(`${address}: change.after is missing or unresolved.`);
    } else if (canonicalJson(change.change.after) !== canonicalJson(plannedResource.values)) {
      violations.push(`${address}: resource_changes after values do not match planned_values.`);
    }
    if (!isPlainObject(change.change.after_unknown)) {
      violations.push(`${address}: after_unknown must be a plain object.`);
    } else if (hasTrue(change.change.after_unknown)) {
      violations.push(`${address}: unresolved after values are prohibited for every managed resource.`);
    }
    if (hasTrue(change.change.before_sensitive) || hasTrue(change.change.after_sensitive)) violations.push(`${address}: sensitive plan values are prohibited in Phase G1.`);
  }
}

function requireEqual(actual, expected, label, violations) {
  if (actual !== expected) violations.push(`${label} must be exactly ${JSON.stringify(expected)}.`);
}

function validateProjectService(resource, expectedService, review, violations) {
  if (!resource) return;
  const values = resource?.values || {};
  rejectUnknownFields(values, PROJECT_SERVICE_FIELDS, `${resource.address} values`, violations);
  requireEqual(values.project, review.projectId, `${resource.address} project`, violations);
  requireEqual(values.service, expectedService, `${resource.address} service`, violations);
  requireEqual(values.disable_on_destroy, false, `${resource.address} disable_on_destroy`, violations);
  if (values.disable_dependent_services !== undefined && values.disable_dependent_services !== null && values.disable_dependent_services !== false) violations.push(`${resource.address}: dependent service disablement is prohibited.`);
  if (values.check_if_service_has_usage_on_destroy !== undefined && values.check_if_service_has_usage_on_destroy !== null && values.check_if_service_has_usage_on_destroy !== false) violations.push(`${resource.address}: service use probing on destroy is outside Phase G1.`);
  requireEmpty(values.timeouts, `${resource.address} timeouts`, violations);
}

function validateServiceAccount(resource, { accountId, displayName, email, review }, violations) {
  if (!resource) return;
  const values = resource?.values || {};
  rejectUnknownFields(values, SERVICE_ACCOUNT_FIELDS, `${resource.address} values`, violations);
  requireEqual(values.project, review.projectId, `${resource.address} project`, violations);
  requireEqual(values.account_id, accountId, `${resource.address} account_id`, violations);
  requireEqual(values.display_name, displayName, `${resource.address} display_name`, violations);
  if (values.email !== undefined && values.email !== null) requireEqual(values.email, email, `${resource.address} email`, violations);
  if (values.name !== undefined && values.name !== null) requireEqual(values.name, `projects/${review.projectId}/serviceAccounts/${email}`, `${resource.address} name`, violations);
  if (values.id !== undefined && values.id !== null) requireEqual(values.id, `projects/${review.projectId}/serviceAccounts/${email}`, `${resource.address} id`, violations);
  if (values.disabled !== undefined && values.disabled !== null && values.disabled !== false) violations.push(`${resource.address}: service account must not be disabled.`);
  if (values.member !== undefined && values.member !== null) requireEqual(values.member, `serviceAccount:${email}`, `${resource.address} member`, violations);
  if (values.description !== undefined && values.description !== null && values.description !== '') violations.push(`${resource.address}: description override is not allowed.`);
  requireEmpty(values.timeouts, `${resource.address} timeouts`, violations);
}

function validatedServiceUri(value, label, violations, serviceName, region, projectNumber) {
  if (typeof value !== 'string') {
    violations.push(`${label} is missing or unresolved.`);
    return null;
  }
  try {
    const parsed = new URL(value);
    const expectedHost = `${serviceName}-${projectNumber}.${region}.run.app`;
    if (parsed.protocol !== 'https:' || parsed.hostname !== expectedHost || parsed.username || parsed.password || parsed.port || (parsed.pathname !== '' && parsed.pathname !== '/') || parsed.search || parsed.hash) throw new TypeError('invalid Cloud Run URI');
    return value.replace(/\/$/u, '');
  } catch {
    violations.push(`${label} must be one canonical HTTPS run.app service URI.`);
    return null;
  }
}

function validateCloudRun(resource, review, reviewedScale, violations) {
  const values = resource?.values || {};
  rejectUnknownFields(values, CLOUD_RUN_FIELDS, `${resource.address} values`, violations);
  requireEqual(values.project, review.projectId, `${resource.address} project`, violations);
  requireEqual(values.location, review.region, `${resource.address} location`, violations);
  requireEqual(values.name, review.serviceName, `${resource.address} name`, violations);
  requireEqual(values.ingress, 'INGRESS_TRAFFIC_INTERNAL_ONLY', `${resource.address} ingress`, violations);
  requireEqual(values.deletion_protection, true, `${resource.address} deletion_protection`, violations);
  if (values.invoker_iam_disabled !== undefined && values.invoker_iam_disabled !== null && values.invoker_iam_disabled !== false) violations.push(`${resource.address}: invoker IAM cannot be disabled.`);
  if (values.iap_enabled !== undefined && values.iap_enabled !== null && values.iap_enabled !== false) violations.push(`${resource.address}: unreviewed IAP configuration is prohibited.`);
  requireEmpty(values.traffic, `${resource.address} traffic override`, violations);
  requireEmpty(values.annotations, `${resource.address} service annotations`, violations);
  requireEmpty(values.labels, `${resource.address} service labels`, violations);
  requireEmpty(values.custom_audiences, `${resource.address} custom audiences`, violations);
  requireEmpty(values.binary_authorization, `${resource.address} binary authorization override`, violations);
  requireEmpty(values.build_config, `${resource.address} build configuration`, violations);

  const template = exactSingleton(values.template, `${resource.address} template`, violations);
  if (!template) return null;
  rejectUnknownFields(template, TEMPLATE_FIELDS, `${resource.address} template`, violations);
  const evidenceAnnotation = { 'ruvnet.dev/image-evidence-digest': review.approvedImageEvidenceDigest };
  if (canonicalJson(template.annotations) !== canonicalJson(evidenceAnnotation)) violations.push(`${resource.address}: template annotation must bind the reviewed image evidence digest.`);
  requireEmpty(template.labels, `${resource.address} template labels`, violations);
  requireEmpty(template.vpc_access, `${resource.address} VPC access`, violations);
  requireEmpty(template.encryption_key, `${resource.address} encryption key override`, violations);
  requireEmpty(template.node_selector, `${resource.address} node selector`, violations);
  requireEmpty(template.revision, `${resource.address} revision override`, violations);
  if (template.execution_environment !== undefined && template.execution_environment !== null && template.execution_environment !== 'EXECUTION_ENVIRONMENT_GEN2') violations.push(`${resource.address}: execution environment must remain Gen 2.`);
  if (template.max_instance_request_concurrency !== undefined && template.max_instance_request_concurrency !== null && ![0, 80].includes(template.max_instance_request_concurrency)) violations.push(`${resource.address}: request concurrency override is prohibited.`);
  if (template.session_affinity !== undefined && template.session_affinity !== null && template.session_affinity !== false) violations.push(`${resource.address}: session affinity is prohibited.`);
  if (template.gpu_zonal_redundancy_disabled !== undefined && template.gpu_zonal_redundancy_disabled !== null && template.gpu_zonal_redundancy_disabled !== false) violations.push(`${resource.address}: GPU redundancy override is prohibited.`);
  requireEqual(template.service_account, expectedRuntimeEmail(review.projectId), `${resource.address} runtime service account`, violations);
  requireEqual(template.timeout, '10s', `${resource.address} timeout`, violations);

  const scaling = exactSingleton(template.scaling, `${resource.address} scaling`, violations);
  if (scaling) {
    rejectUnknownFields(scaling, SCALING_FIELDS, `${resource.address} scaling`, violations);
    const minimum = scaling.min_instance_count;
    const maximum = scaling.max_instance_count;
    if (!Number.isInteger(minimum) || minimum < 0 || minimum > 3) violations.push(`${resource.address}: minimum instance count is outside zero through three.`);
    if (!Number.isInteger(maximum) || maximum < 1 || maximum > 3) violations.push(`${resource.address}: maximum instance count exceeds three or is invalid.`);
    if (Number.isInteger(minimum) && Number.isInteger(maximum) && minimum > maximum) violations.push(`${resource.address}: minimum instance count exceeds maximum.`);
    if (reviewedScale) {
      requireEqual(minimum, reviewedScale.minimum, `${resource.address} planned minimum scale`, violations);
      requireEqual(maximum, reviewedScale.maximum, `${resource.address} planned maximum scale`, violations);
    }
  }

  const container = exactSingleton(template.containers, `${resource.address} containers`, violations);
  if (!container) return null;
  rejectUnknownFields(container, CONTAINER_FIELDS, `${resource.address} container`, violations);
  if (!ARTIFACT_REGISTRY_DIGEST.test(container.image || '')) violations.push(`${resource.address}: image is not one full Artifact Registry digest URI.`);
  requireEqual(container.image, review.approvedImage, `${resource.address} image`, violations);
  if (!emptyCollection(container.env)) violations.push(`${resource.address}: runtime environment variables are prohibited in Phase G1.`);
  if (!emptyCollection(container.volume_mounts) || !emptyCollection(template.volumes)) violations.push(`${resource.address}: volumes and secret mounts are prohibited in Phase G1.`);
  requireEmpty(container.command, `${resource.address} container command override`, violations);
  requireEmpty(container.args, `${resource.address} container argument override`, violations);
  requireEmpty(container.depends_on, `${resource.address} container dependency override`, violations);
  requireEmpty(container.ports, `${resource.address} container port override`, violations);
  if (container.base_image_uri !== undefined && container.base_image_uri !== null && container.base_image_uri !== '') violations.push(`${resource.address}: base image override is prohibited.`);
  if (container.working_dir !== undefined && container.working_dir !== null && container.working_dir !== '') violations.push(`${resource.address}: working directory override is prohibited.`);

  const resources = exactSingleton(container.resources, `${resource.address} container resources`, violations);
  if (resources) {
    rejectUnknownFields(resources, CONTAINER_RESOURCE_FIELDS, `${resource.address} container resources`, violations);
    requireEqual(resources.cpu_idle, true, `${resource.address} cpu_idle`, violations);
    if (canonicalJson(resources.limits) !== canonicalJson({ cpu: '1', memory: '512Mi' })) violations.push(`${resource.address}: limits must be exactly 1 CPU and 512Mi memory.`);
    if (resources.startup_cpu_boost !== undefined && resources.startup_cpu_boost !== null && resources.startup_cpu_boost !== false) violations.push(`${resource.address}: startup CPU boost is prohibited.`);
  }

  const probe = exactSingleton(container.startup_probe, `${resource.address} startup_probe`, violations);
  if (probe) {
    rejectUnknownFields(probe, PROBE_FIELDS, `${resource.address} startup_probe`, violations);
    requireEqual(probe.initial_delay_seconds, 1, `${resource.address} startup probe initial delay`, violations);
    requireEqual(probe.timeout_seconds, 2, `${resource.address} startup probe timeout`, violations);
    requireEqual(probe.period_seconds, 5, `${resource.address} startup probe period`, violations);
    requireEqual(probe.failure_threshold, 6, `${resource.address} startup probe failure threshold`, violations);
    if (!emptyCollection(probe.grpc) || !emptyCollection(probe.tcp_socket)) violations.push(`${resource.address}: startup probe must use HTTP only.`);
    const httpGet = exactSingleton(probe.http_get, `${resource.address} startup probe http_get`, violations);
    if (httpGet) {
      rejectUnknownFields(httpGet, HTTP_GET_FIELDS, `${resource.address} startup probe http_get`, violations);
      requireEqual(httpGet.path, '/healthz', `${resource.address} startup probe path`, violations);
      requireEqual(httpGet.port, 8080, `${resource.address} startup probe port`, violations);
      requireEmpty(httpGet.http_headers, `${resource.address} startup probe headers`, violations);
    }
  }
  if (!review.heartbeatEnabled) return null;
  const serviceUri = validatedServiceUri(values.uri, `${resource.address} URI`, violations, review.serviceName, review.region, review.projectNumber);
  requireEqual(serviceUri, review.heartbeatServiceUri, `${resource.address} reviewed URI`, violations);
  return serviceUri;
}

function validateHeartbeat(resources, review, serviceUri, violations) {
  const heartbeatEmail = expectedHeartbeatEmail(review.projectId);
  const heartbeatIdentity = resources.get('google_service_account.heartbeat[0]');
  const invoker = resources.get('google_cloud_run_v2_service_iam_member.heartbeat_invoker[0]');
  const schedulerApi = resources.get('google_project_service.scheduler[0]');
  const scheduler = resources.get('google_cloud_scheduler_job.heartbeat[0]');

  validateServiceAccount(heartbeatIdentity, {
    accountId: 'ruv-social-heartbeat',
    displayName: 'RuV social Cloud Run heartbeat invoker',
    email: heartbeatEmail,
    review,
  }, violations);

  if (invoker) {
    const iam = invoker.values || {};
    rejectUnknownFields(iam, IAM_MEMBER_FIELDS, `${invoker.address} values`, violations);
    requireEqual(iam.project, review.projectId, `${invoker.address} project`, violations);
    requireEqual(iam.location, review.region, `${invoker.address} location`, violations);
    requireEqual(iam.name, review.serviceName, `${invoker.address} name`, violations);
    requireEqual(iam.role, 'roles/run.invoker', `${invoker.address} role`, violations);
    requireEqual(iam.member, `serviceAccount:${heartbeatEmail}`, `${invoker.address} member`, violations);
    requireEmpty(iam.condition, `${invoker.address} condition`, violations);
  }

  validateProjectService(schedulerApi, 'cloudscheduler.googleapis.com', review, violations);

  if (!scheduler) return;
  const values = scheduler.values || {};
  rejectUnknownFields(values, SCHEDULER_FIELDS, `${scheduler.address} values`, violations);
  requireEqual(values.project, review.projectId, `${scheduler.address} project`, violations);
  requireEqual(values.region, review.region, `${scheduler.address} region`, violations);
  requireEqual(values.name, `${review.serviceName}-heartbeat`, `${scheduler.address} name`, violations);
  requireEqual(values.description, 'Authenticated health check. It does not provide perpetual execution.', `${scheduler.address} description`, violations);
  requireEqual(values.schedule, '*/5 * * * *', `${scheduler.address} schedule`, violations);
  requireEqual(values.time_zone, 'UTC', `${scheduler.address} time_zone`, violations);
  if (values.paused !== undefined && values.paused !== null && values.paused !== false) violations.push(`${scheduler.address}: enabled heartbeat cannot be paused.`);
  requireEmpty(values.retry_config, `${scheduler.address} retry configuration`, violations);
  requireEmpty(values.timeouts, `${scheduler.address} timeouts`, violations);
  if (values.attempt_deadline !== undefined && values.attempt_deadline !== null && values.attempt_deadline !== '180s') violations.push(`${scheduler.address}: attempt deadline override is prohibited.`);

  const target = exactSingleton(values.http_target, `${scheduler.address} http_target`, violations);
  if (!target) return;
  rejectUnknownFields(target, HTTP_TARGET_FIELDS, `${scheduler.address} http_target`, violations);
  requireEqual(target.http_method, 'GET', `${scheduler.address} HTTP method`, violations);
  requireEqual(target.uri, `${review.heartbeatServiceUri}/healthz`, `${scheduler.address} URI`, violations);
  const oidc = exactSingleton(target.oidc_token, `${scheduler.address} oidc_token`, violations);
  if (oidc) {
    rejectUnknownFields(oidc, OIDC_FIELDS, `${scheduler.address} oidc_token`, violations);
    requireEqual(oidc.service_account_email, heartbeatEmail, `${scheduler.address} OIDC service account`, violations);
    requireEqual(oidc.audience, review.heartbeatServiceUri, `${scheduler.address} OIDC audience`, violations);
  }
  if (!serviceUri) violations.push(`${scheduler.address}: target cannot be verified without a resolved Cloud Run URI.`);
  if (!emptyCollection(target.headers) || target.body !== undefined && target.body !== null) violations.push(`${scheduler.address}: heartbeat headers and body are prohibited.`);
  if (!emptyCollection(target.oauth_token)) violations.push(`${scheduler.address}: OAuth target credentials are prohibited; use the exact OIDC block.`);
}

function validatePublicAndIam(resources, violations) {
  for (const resource of resources.values()) {
    const values = resource?.values || {};
    if (/_iam_policy$/u.test(resource?.type || '')) violations.push(`${resource.address}: authoritative IAM policy resources are prohibited.`);
    if (values.role !== undefined && values.role !== 'roles/run.invoker') violations.push(`${resource.address}: IAM role is outside the Phase G1 allowlist.`);
    const members = [];
    if (typeof values.member === 'string') members.push(values.member);
    if (Array.isArray(values.members)) members.push(...values.members.filter((value) => typeof value === 'string'));
    for (const member of members) if (PUBLIC_MEMBERS.has(member)) violations.push(`${resource.address}: public IAM principal is prohibited.`);
  }
}

export function evaluatePlan(plan, review, { now = new Date() } = {}) {
  if (!isPlainObject(plan)) throw new TypeError('Terraform plan JSON must be an object');
  const violations = [];
  if (plan.errored !== false) violations.push('Terraform plan errored must be exactly false.');
  if (plan.complete !== true) violations.push('Terraform plan complete must be exactly true.');
  if (plan.applyable !== true) violations.push('Terraform plan applyable must be exactly true.');
  if (typeof plan.format_version !== 'string' || !/^1\.[0-9]+$/u.test(plan.format_version)) violations.push('Terraform plan format version must be an explicit supported 1.x value.');
  if (Array.isArray(plan.resource_drift) && plan.resource_drift.length > 0) violations.push('Terraform resource drift must be resolved before Phase G1 review.');
  if (Array.isArray(plan.deferred_changes) && plan.deferred_changes.length > 0) violations.push('Terraform deferred changes are prohibited.');
  validateConfiguration(plan, violations);
  const acceptedReview = validateReview(plan, review, now, violations);
  const reviewedScale = acceptedReview ? validatePlanVariables(plan, acceptedReview, violations) : null;
  const heartbeatEnabled = acceptedReview?.heartbeatEnabled === true;
  const expected = expectedResources(heartbeatEnabled);
  const plannedResources = collectResources(plan.planned_values?.root_module);
  const resources = indexExactResources(plannedResources, expected, 'planned_values', violations);
  validateResourceChanges(plan, resources, expected, violations);

  if (acceptedReview) {
    for (const requiredService of REQUIRED_SERVICES) {
      const address = `google_project_service.required["${requiredService}"]`;
      const resource = resources.get(address);
      if (resource) validateProjectService(resource, requiredService, acceptedReview, violations);
    }
    const runtimeIdentity = resources.get('google_service_account.control_plane');
    if (runtimeIdentity) validateServiceAccount(runtimeIdentity, {
      accountId: 'ruv-social-control',
      displayName: 'RuV social read only control plane',
      email: expectedRuntimeEmail(acceptedReview.projectId),
      review: acceptedReview,
    }, violations);
    const service = resources.get('google_cloud_run_v2_service.control_plane');
    const serviceUri = service ? validateCloudRun(service, acceptedReview, reviewedScale, violations) : null;
    if (heartbeatEnabled) validateHeartbeat(resources, acceptedReview, serviceUri, violations);
  }
  validatePublicAndIam(resources, violations);

  return Object.freeze({
    schema: 'SocialGcpPlanPolicyV1',
    ok: violations.length === 0,
    violations,
    planDigest: sha256(plan),
    reviewAuthorityVerified: false,
  });
}

function readBoundedJson(path, name) {
  if (path !== '-') {
    const size = statSync(path).size;
    if (size > MAX_PLAN_BYTES) throw new RangeError(`${name} JSON exceeds 10 MiB`);
  }
  const content = readFileSync(path === '-' ? 0 : path);
  if (content.length > MAX_PLAN_BYTES) throw new RangeError(`${name} JSON exceeds 10 MiB`);
  return JSON.parse(content.toString('utf8'));
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try {
    const planPath = process.argv[2] || '-';
    const reviewPath = process.argv[3];
    if (!reviewPath || reviewPath === '-') throw new TypeError('A separate reviewed policy JSON file is required');
    const result = evaluatePlan(readBoundedJson(planPath, 'Terraform plan'), readBoundedJson(reviewPath, 'Plan review'));
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    if (!result.ok) process.exitCode = 1;
  } catch (error) {
    process.stderr.write(`GCP plan policy failed: ${error instanceof Error ? error.message : 'unknown error'}\n`);
    process.exitCode = 1;
  }
}
