import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { evaluatePlan } from '../deploy/gcp/check-plan.mjs';
import { sha256 } from '../src/canonical.js';

function file(path) {
  return readFileSync(new URL(path, import.meta.url), 'utf8');
}

const main = file('../deploy/gcp/main.tf');
const variables = file('../deploy/gcp/variables.tf');
const container = file('../Containerfile');
const projectId = 'example-project';
const projectNumber = '123456789012';
const region = 'us-central1';
const serviceName = 'ruv-social-control';
const serviceUri = 'https://ruv-social-control-123456789012.us-central1.run.app';
const runtimeEmail = `ruv-social-control@${projectId}.iam.gserviceaccount.com`;
const heartbeatEmail = `ruv-social-heartbeat@${projectId}.iam.gserviceaccount.com`;
const image = `${region}-docker.pkg.dev/${projectId}/social/ruv-social@sha256:${'a'.repeat(64)}`;
const now = new Date('2026-08-29T12:00:00.000Z');
const approvedImageEvidence = Object.freeze({
  schema: 'SocialImageEvidenceV1',
  image,
  sourceRevisionDigest: `sha256:${'c'.repeat(64)}`,
  sbomDigest: `sha256:${'d'.repeat(64)}`,
  vulnerabilityScanDigest: `sha256:${'e'.repeat(64)}`,
  provenanceDigest: `sha256:${'f'.repeat(64)}`,
  recordedAt: '2026-08-29T10:00:00.000Z',
});
const imageEvidenceDigest = sha256(approvedImageEvidence);
const requiredApiNames = Object.freeze([
  'artifactregistry.googleapis.com',
  'cloudresourcemanager.googleapis.com',
  'iam.googleapis.com',
  'run.googleapis.com',
  'serviceusage.googleapis.com',
]);

function review(input, overrides = {}) {
  const heartbeatEnabled = input.planned_values.root_module.resources.some(({ address }) => address === 'google_cloud_scheduler_job.heartbeat[0]');
  return {
    schema: 'SocialGcpPlanReviewV1',
    planDigest: sha256(input),
    projectId,
    projectNumber,
    region,
    serviceName,
    approvedImage: image,
    approvedImageEvidence,
    approvedImageEvidenceDigest: imageEvidenceDigest,
    heartbeatEnabled,
    heartbeatServiceUri: heartbeatEnabled ? serviceUri : null,
    reviewedAt: '2026-08-29T11:00:00.000Z',
    expiresAt: '2026-08-30T11:00:00.000Z',
    ...overrides,
  };
}

function managed(address, type, values) {
  return {
    address,
    mode: 'managed',
    type,
    provider_name: 'google',
    sensitive_values: {},
    values,
  };
}

function configuration() {
  const declarations = [
    ['google_project_service.required', 'google_project_service'],
    ['google_service_account.control_plane', 'google_service_account'],
    ['google_service_account.heartbeat', 'google_service_account'],
    ['google_cloud_run_v2_service.control_plane', 'google_cloud_run_v2_service'],
    ['google_cloud_run_v2_service_iam_member.heartbeat_invoker', 'google_cloud_run_v2_service_iam_member'],
    ['google_project_service.scheduler', 'google_project_service'],
    ['google_cloud_scheduler_job.heartbeat', 'google_cloud_scheduler_job'],
  ].map(([address, type]) => ({ address, mode: 'managed', type, provider_config_key: 'google', provisioners: [] }));
  return {
    provider_config: { google: { name: 'google', full_name: 'registry.terraform.io/hashicorp/google' } },
    root_module: { resources: declarations, module_calls: {} },
  };
}

function requiredServices() {
  return requiredApiNames.map((service) => managed(
    `google_project_service.required["${service}"]`,
    'google_project_service',
    { project: projectId, service, disable_on_destroy: false },
  ));
}

function syncChanges(input, actions = ['create']) {
  input.resource_changes = input.planned_values.root_module.resources.map((resource) => ({
    address: resource.address,
    mode: resource.mode,
    type: resource.type,
    provider_name: resource.provider_name,
    change: {
      actions: [...actions],
      before: null,
      after: structuredClone(resource.values),
      after_unknown: {},
      before_sensitive: false,
      after_sensitive: false,
    },
  }));
  return input;
}

function plan({ maximumInstances = 3, runtimeImage = image, heartbeat = false } = {}) {
  const resources = [
    ...requiredServices(),
    managed('google_service_account.control_plane', 'google_service_account', {
      project: projectId,
      account_id: 'ruv-social-control',
      display_name: 'RuV social read only control plane',
      email: runtimeEmail,
    }),
    managed('google_cloud_run_v2_service.control_plane', 'google_cloud_run_v2_service', {
      project: projectId,
      location: region,
      name: serviceName,
      uri: serviceUri,
      deletion_protection: true,
      ingress: 'INGRESS_TRAFFIC_INTERNAL_ONLY',
      template: [{
        annotations: { 'ruvnet.dev/image-evidence-digest': imageEvidenceDigest },
        service_account: runtimeEmail,
        timeout: '10s',
        scaling: [{ min_instance_count: 0, max_instance_count: maximumInstances }],
        containers: [{
          image: runtimeImage,
          env: [],
          volume_mounts: [],
          resources: [{ cpu_idle: true, limits: { cpu: '1', memory: '512Mi' } }],
          startup_probe: [{
            initial_delay_seconds: 1,
            timeout_seconds: 2,
            period_seconds: 5,
            failure_threshold: 6,
            http_get: [{ path: '/healthz', port: 8080 }],
            grpc: [],
            tcp_socket: [],
          }],
        }],
        volumes: [],
      }],
    }),
  ];

  if (heartbeat) {
    resources.push(
      managed('google_service_account.heartbeat[0]', 'google_service_account', {
        project: projectId,
        account_id: 'ruv-social-heartbeat',
        display_name: 'RuV social Cloud Run heartbeat invoker',
        email: heartbeatEmail,
      }),
      managed('google_cloud_run_v2_service_iam_member.heartbeat_invoker[0]', 'google_cloud_run_v2_service_iam_member', {
        project: projectId,
        location: region,
        name: serviceName,
        role: 'roles/run.invoker',
        member: `serviceAccount:${heartbeatEmail}`,
      }),
      managed('google_project_service.scheduler[0]', 'google_project_service', {
        project: projectId,
        service: 'cloudscheduler.googleapis.com',
        disable_on_destroy: false,
      }),
      managed('google_cloud_scheduler_job.heartbeat[0]', 'google_cloud_scheduler_job', {
        project: projectId,
        region,
        name: `${serviceName}-heartbeat`,
        description: 'Authenticated health check. It does not provide perpetual execution.',
        schedule: '*/5 * * * *',
        time_zone: 'UTC',
        http_target: [{
          http_method: 'GET',
          uri: `${serviceUri}/healthz`,
          headers: {},
          body: null,
          oidc_token: [{ service_account_email: heartbeatEmail, audience: serviceUri }],
        }],
      }),
    );
  }

  return syncChanges({
    format_version: '1.2',
    complete: true,
    errored: false,
    applyable: true,
    configuration: configuration(),
    variables: {
      project_id: { value: projectId },
      region: { value: region },
      service_name: { value: serviceName },
      container_image: { value: runtimeImage },
      image_evidence_digest: { value: imageEvidenceDigest },
      minimum_instances: { value: 0 },
      maximum_instances: { value: maximumInstances },
      enable_heartbeat: { value: heartbeat },
    },
    planned_values: { root_module: { resources } },
    resource_changes: [],
  });
}

function messages(result) {
  return result.violations.join('\n');
}

test('container build requires an operator supplied base image and drops root', () => {
  assert.match(container, /^ARG NODE_IMAGE$/mu);
  assert.match(container, /^FROM \$\{NODE_IMAGE\}$/mu);
  assert.match(container, /^COPY \.harness \.\/\.harness$/mu);
  assert.match(container, /^USER node$/mu);
  assert.doesNotMatch(container, /(?:latest|npm install|curl|wget)/u);
});

test('GCP source is internal, bounded, deterministic, and least authority', () => {
  assert.match(main, /^provider "google" \{\}$/mu);
  assert.doesNotMatch(main, /provider "google" \{[\s\S]*?(?:credentials|access_token|impersonate_service_account|custom_endpoint)/u);
  assert.match(main, /"iam\.googleapis\.com"/u);
  assert.doesNotMatch(main, /"(?:cloudbuild|secretmanager)\.googleapis\.com"/u);
  assert.match(main, /control_plane_email\s+=\s+"ruv-social-control@\$\{var\.project_id\}\.iam\.gserviceaccount\.com"/u);
  assert.match(main, /heartbeat_email\s+=\s+"ruv-social-heartbeat@\$\{var\.project_id\}\.iam\.gserviceaccount\.com"/u);
  assert.match(main, /INGRESS_TRAFFIC_INTERNAL_ONLY/u);
  assert.match(main, /deletion_protection = true/u);
  assert.match(main, /timeout\s+=\s+"10s"/u);
  assert.match(main, /cpu\s+=\s+"1"/u);
  assert.match(main, /memory\s+=\s+"512Mi"/u);
  assert.match(main, /cpu_idle = true/u);
  assert.match(main, /ruvnet\.dev\/image-evidence-digest/u);
  assert.match(variables, /variable "image_evidence_digest"/u);
  assert.match(main, /path = "\/healthz"/u);
  assert.match(main, /port = 8080/u);
  assert.match(main, /var\.minimum_instances <= var\.maximum_instances/u);
  assert.match(main, /startswith\(var\.container_image, "\$\{var\.region\}-docker\.pkg\.dev\/\$\{var\.project_id\}\/"\)/u);
  assert.match(main, /resource "google_cloud_run_v2_service" "control_plane"[\s\S]*?depends_on = \[[\s\S]*?google_service_account\.control_plane[\s\S]*?\]/u);
  assert.match(main, /depends_on = \[google_service_account\.heartbeat\]/u);
  assert.doesNotMatch(main, /roles\/(?:owner|editor)/u);
  assert.doesNotMatch(main, /allUsers|allAuthenticatedUsers/u);
  assert.doesNotMatch(main, /secret_data|secret_value/u);
  assert.match(variables, /default\s+=\s+0/u);
  assert.match(variables, /var\.maximum_instances >= 1 && var\.maximum_instances <= 3 && floor\(var\.maximum_instances\) == var\.maximum_instances/u);
  assert.equal(variables.includes(String.raw`-docker\\.pkg\\.dev`), true);
  assert.match(variables, /@sha256:\[0-9a-f\]\{64\}/u);
});

test('heartbeat source is optional and pins the exact authenticated request', () => {
  assert.match(main, /var\.enable_heartbeat \? 1 : 0/u);
  assert.match(main, /schedule\s+=\s+"\*\/5 \* \* \* \*"/u);
  assert.match(main, /time_zone\s+=\s+"UTC"/u);
  assert.match(main, /http_method = "GET"/u);
  assert.match(main, /uri\s+=\s+"\$\{google_cloud_run_v2_service\.control_plane\.uri\}\/healthz"/u);
  assert.match(main, /oidc_token/u);
  assert.match(main, /audience\s+=\s+google_cloud_run_v2_service\.control_plane\.uri/u);
});

test('GCP plan policy accepts the exact reviewed graph with heartbeat absent', () => {
  const input = plan();
  assert.deepEqual(evaluatePlan(input, review(input), { now }), {
    schema: 'SocialGcpPlanPolicyV1',
    ok: true,
    violations: [],
    planDigest: sha256(input),
    reviewAuthorityVerified: false,
  });
});

test('GCP plan policy accepts the exact reviewed heartbeat graph', () => {
  const input = plan({ heartbeat: true });
  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, true, messages(result));
  assert.equal(result.reviewAuthorityVerified, false);
});

test('review binds canonical plan digest, project, region, service, heartbeat, image evidence, and time', () => {
  const input = plan();
  const approved = review(input);
  input.planned_values.root_module.resources.find(({ address }) => address === 'google_cloud_run_v2_service.control_plane').values.name = 'attacker-service';
  let result = evaluatePlan(input, approved, { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /canonical Terraform plan/u);

  const fresh = plan();
  result = evaluatePlan(fresh, review(fresh, {
    projectId: 'attacker-project',
    projectNumber: '123',
    region: 'europe-west1',
    serviceName: 'attacker-service',
    heartbeatEnabled: true,
    approvedImageEvidenceDigest: 'unverified',
    reviewedAt: '2026-08-29T13:00:00.000Z',
    expiresAt: '2026-08-31T13:00:00.000Z',
  }), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /evidence digest is invalid/u);
  assert.match(messages(result), /project number must contain exactly 12 digits/u);
  assert.match(messages(result), /future/u);
  assert.match(messages(result), /no more than 24 hours/u);
  assert.match(messages(result), /required resource google_cloud_scheduler_job\.heartbeat\[0\] is missing/u);
  assert.match(messages(result), /project must be exactly/u);
  assert.match(messages(result), /location must be exactly/u);
  assert.match(messages(result), /name must be exactly/u);

  const evidenceTamper = plan();
  const tamperedEvidence = { ...approvedImageEvidence, sbomDigest: `sha256:${'9'.repeat(64)}` };
  result = evaluatePlan(evidenceTamper, review(evidenceTamper, { approvedImageEvidence: tamperedEvidence }), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /canonical evidence record/u);

  const crossProject = plan();
  const attackerImage = `${region}-docker.pkg.dev/attacker-project/social/ruv-social@sha256:${'8'.repeat(64)}`;
  result = evaluatePlan(crossProject, review(crossProject, { approvedImage: attackerImage }), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /Reviewed image project must be exactly "example-project"/u);
  assert.match(messages(result), /approvedImageEvidence image/u);

  const annotationTamper = plan();
  annotationTamper.planned_values.root_module.resources.find(({ address }) => address === 'google_cloud_run_v2_service.control_plane').values.template[0].annotations['ruvnet.dev/image-evidence-digest'] = `sha256:${'7'.repeat(64)}`;
  syncChanges(annotationTamper);
  result = evaluatePlan(annotationTamper, review(annotationTamper), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /template annotation must bind the reviewed image evidence digest/u);
});

test('previous attacker plan fails despite matching its reviewed plan digest', () => {
  const input = plan({ heartbeat: true });
  const service = input.planned_values.root_module.resources.find(({ address }) => address === 'google_cloud_run_v2_service.control_plane');
  const scheduler = input.planned_values.root_module.resources.find(({ address }) => address === 'google_cloud_scheduler_job.heartbeat[0]');
  service.values.location = 'europe-west1';
  service.values.name = 'attacker-service';
  service.values.uri = 'https://attacker.run.app';
  service.values.invoker_iam_disabled = true;
  service.values.backdoor = true;
  service.values.traffic = [{ percent: 100, revision: 'malicious-revision', type: 'TRAFFIC_TARGET_ALLOCATION_TYPE_REVISION' }];
  service.values.template[0].vpc_access = [{ egress: 'ALL_TRAFFIC', network_interfaces: [{ network: 'attacker' }] }];
  service.values.template[0].containers[0].command = ['/attacker'];
  service.values.template[0].containers[0].args = ['exfiltrate'];
  service.values.template[0].containers[0].resources[0] = { cpu_idle: false, limits: { cpu: '32', memory: '128Gi' } };
  scheduler.values.region = 'europe-west1';
  scheduler.values.name = 'attacker-heartbeat';
  scheduler.values.schedule = '* * * * *';
  scheduler.values.time_zone = 'America/Toronto';
  scheduler.values.http_target[0].http_method = 'POST';
  scheduler.values.http_target[0].uri = 'https://attacker.invalid/collect';
  scheduler.values.http_target[0].oidc_token[0].audience = 'https://attacker.invalid';
  scheduler.values.http_target[0].headers = { Authorization: 'Bearer attacker' };
  scheduler.values.http_target[0].body = 'ZXhmaWx0cmF0ZQ==';
  scheduler.values.paused = true;
  scheduler.values.retry_config = [{ retry_count: 99, min_backoff_duration: '1s' }];
  syncChanges(input);

  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /location must be exactly "us-central1"/u);
  assert.match(messages(result), /name must be exactly "ruv-social-control"/u);
  assert.match(messages(result), /cpu_idle must be exactly true/u);
  assert.match(messages(result), /exactly 1 CPU and 512Mi memory/u);
  assert.match(messages(result), /invoker IAM cannot be disabled/u);
  assert.match(messages(result), /values\.backdoor is not allowed/u);
  assert.match(messages(result), /traffic override must be absent or empty/u);
  assert.match(messages(result), /VPC access must be absent or empty/u);
  assert.match(messages(result), /container command override must be absent or empty/u);
  assert.match(messages(result), /container argument override must be absent or empty/u);
  assert.match(messages(result), /URI must be one canonical HTTPS run\.app service URI/u);
  assert.match(messages(result), /schedule must be exactly "\*\/5 \* \* \* \*"/u);
  assert.match(messages(result), /HTTP method must be exactly "GET"/u);
  assert.match(messages(result), /URI must be exactly/u);
  assert.match(messages(result), /OIDC audience must be exactly/u);
  assert.match(messages(result), /headers and body are prohibited/u);
  assert.match(messages(result), /enabled heartbeat cannot be paused/u);
  assert.match(messages(result), /retry configuration must be absent or empty/u);
});

test('heartbeat URI must bind the reviewed 12 digit project number', () => {
  const input = plan({ heartbeat: true });
  const wrongUri = `https://${serviceName}-999999999999.${region}.run.app`;
  const service = input.planned_values.root_module.resources.find(({ address }) => address === 'google_cloud_run_v2_service.control_plane');
  const scheduler = input.planned_values.root_module.resources.find(({ address }) => address === 'google_cloud_scheduler_job.heartbeat[0]');
  service.values.uri = wrongUri;
  scheduler.values.http_target[0].uri = `${wrongUri}/healthz`;
  scheduler.values.http_target[0].oidc_token[0].audience = wrongUri;
  syncChanges(input);
  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /URI must be one canonical HTTPS run\.app service URI/u);
  assert.match(messages(result), /URI must be exactly/u);
  assert.match(messages(result), /OIDC audience must be exactly/u);
});

test('resource graph rejects duplicates, wrong types, unknown resources, and partial heartbeat', () => {
  const input = plan();
  const resources = input.planned_values.root_module.resources;
  resources.find(({ address }) => address === 'google_service_account.control_plane').type = 'google_project_iam_member';
  resources.push(structuredClone(resources[0]));
  resources.push(managed('google_cloud_run_v2_job.publisher', 'google_cloud_run_v2_job', { project: projectId }));
  resources.push(managed('google_service_account.heartbeat[0]', 'google_service_account', { project: projectId }));
  syncChanges(input);
  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /resource type must be exactly google_service_account/u);
  assert.match(messages(result), /duplicate resource address/u);
  assert.match(messages(result), /outside the exact Phase G1 graph/u);
  assert.match(messages(result), /resource count must be exactly/u);
});

test('resource_changes must match planned values and resolve every critical field', () => {
  const input = plan();
  const change = input.resource_changes.find(({ address }) => address === 'google_cloud_run_v2_service.control_plane');
  const missingUnknown = input.resource_changes.find(({ address }) => address === 'google_service_account.control_plane');
  const malformedUnknown = input.resource_changes.find(({ address }) => address.includes('artifactregistry.googleapis.com'));
  change.change.actions = ['delete', 'create'];
  change.change.after.project = 'attacker-project';
  change.change.after_unknown = { traffic: true };
  change.change.after_sensitive = { template: [{ containers: [{ image: true }] }] };
  delete missingUnknown.change.after_unknown;
  malformedUnknown.change.after_unknown = true;
  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /only allowed action/u);
  assert.match(messages(result), /do not match planned_values/u);
  assert.match(messages(result), /unresolved after values are prohibited for every managed resource/u);
  assert.match(messages(result), /after_unknown must be a plain object/u);
  assert.match(messages(result), /sensitive plan values are prohibited/u);
});

test('scale, image, public IAM, environment, volumes, and incomplete plans fail closed', () => {
  const attackerImage = `${region}-docker.pkg.dev/${projectId}/social/attacker@sha256:${'d'.repeat(64)}`;
  const input = plan({ maximumInstances: 4, runtimeImage: attackerImage, heartbeat: true });
  input.complete = false;
  const service = input.planned_values.root_module.resources.find(({ address }) => address === 'google_cloud_run_v2_service.control_plane');
  const invoker = input.planned_values.root_module.resources.find(({ address }) => address === 'google_cloud_run_v2_service_iam_member.heartbeat_invoker[0]');
  service.values.template[0].containers[0].env = [{ name: 'GITHUB_TOKEN', value: 'not-a-real-secret' }];
  service.values.template[0].volumes = [{ name: 'secret' }];
  invoker.values.member = 'allUsers';
  syncChanges(input);
  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /complete must be exactly true/u);
  assert.match(messages(result), /maximum instance count exceeds three/u);
  assert.match(messages(result), /image must be exactly/u);
  assert.match(messages(result), /environment variables are prohibited/u);
  assert.match(messages(result), /volumes and secret mounts are prohibited/u);
  assert.match(messages(result), /member must be exactly/u);
  assert.match(messages(result), /public IAM principal/u);
});

test('missing or malformed plan completion evidence fails closed', () => {
  const input = plan();
  delete input.complete;
  delete input.errored;
  delete input.applyable;
  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /errored must be exactly false/u);
  assert.match(messages(result), /complete must be exactly true/u);
  assert.match(messages(result), /applyable must be exactly true/u);
});

test('provider, configuration, provisioner, sensitive, and change metadata fail closed', () => {
  const input = plan();
  const planned = input.planned_values.root_module.resources.find(({ address }) => address === 'google_service_account.control_plane');
  const change = input.resource_changes.find(({ address }) => address === 'google_service_account.control_plane');
  planned.provider_name = 'registry.example.invalid/attacker/google';
  planned.sensitive_values = { private_key: true };
  change.provider_name = 'registry.example.invalid/attacker/google';
  change.deposed = 'deadbeef';
  change.change.importing = { id: 'attacker' };
  change.change.generated_config = 'provisioner "local-exec" {}';
  change.change.replace_paths = [['template']];
  change.change.before_identity = { attacker: true };
  change.change.after_identity = { attacker: true };
  input.configuration.provider_config.attacker = { name: 'google', full_name: 'registry.example.invalid/attacker/google' };
  input.configuration.provider_config.google.expressions = { credentials: { constant_value: 'attacker' } };
  input.configuration.root_module.resources[0].provisioners = [{ type: 'local-exec', expressions: { command: { constant_value: 'exfiltrate' } } }];
  input.configuration.root_module.resources[0].connection = { type: 'ssh' };
  input.configuration.root_module.resources.push({
    address: 'null_resource.attacker',
    mode: 'managed',
    type: 'null_resource',
    provider_config_key: 'attacker',
    provisioners: [{ type: 'local-exec' }],
  });
  input.configuration.root_module.module_calls = { attacker: { source: './attacker' } };
  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /exactly one google provider configuration/u);
  assert.match(messages(result), /provider expressions must be absent or empty/u);
  assert.match(messages(result), /\.connection is not allowed/u);
  assert.match(messages(result), /provisioners must be absent or empty/u);
  assert.match(messages(result), /outside the exact Phase G1 source graph/u);
  assert.match(messages(result), /module calls must be absent or empty/u);
  assert.match(messages(result), /provider must be exactly "google"/u);
  assert.match(messages(result), /planned sensitive values are prohibited/u);
  assert.match(messages(result), /deposed instance must be absent or empty/u);
  assert.match(messages(result), /importing must be absent or empty/u);
  assert.match(messages(result), /generated configuration must be absent or empty/u);
  assert.match(messages(result), /replacement paths must be absent or empty/u);
  assert.match(messages(result), /before identity must be absent or empty/u);
  assert.match(messages(result), /after identity must be absent or empty/u);
});

test('plan variables are closed and bind reviewed identity, image, evidence, heartbeat, and scale', () => {
  const input = plan();
  input.variables.github_token = { value: 'attacker' };
  input.variables.project_id = { value: 'attacker-project', hidden: true };
  input.variables.region.value = 'europe-west1';
  input.variables.service_name.value = 'attacker-service';
  input.variables.container_image.value = `${region}-docker.pkg.dev/attacker-project/social/attacker@sha256:${'6'.repeat(64)}`;
  input.variables.image_evidence_digest.value = `sha256:${'5'.repeat(64)}`;
  input.variables.enable_heartbeat.value = true;
  input.variables.minimum_instances.value = 3;
  input.variables.maximum_instances.value = 1;
  const result = evaluatePlan(input, review(input), { now });
  assert.equal(result.ok, false);
  assert.match(messages(result), /variable github_token is not allowed/u);
  assert.match(messages(result), /variable count must be exactly 8/u);
  assert.match(messages(result), /project_id must contain only value/u);
  assert.match(messages(result), /region variable must be exactly/u);
  assert.match(messages(result), /service_name variable must be exactly/u);
  assert.match(messages(result), /container_image variable must be exactly/u);
  assert.match(messages(result), /image_evidence_digest variable must be exactly/u);
  assert.match(messages(result), /enable_heartbeat variable must be exactly false/u);
  assert.match(messages(result), /variable minimum exceeds maximum/u);
  assert.match(messages(result), /planned minimum scale must be exactly 3/u);
  assert.match(messages(result), /planned maximum scale must be exactly 1/u);
});
