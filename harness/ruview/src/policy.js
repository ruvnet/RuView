// SPDX-License-Identifier: MIT
// Executable least-authority policy for CLI/MCP tools.

export const TOOL_POLICY = Object.freeze({
  ruview_onboard: { class: 'read', readOnly: true },
  ruview_claim_check: { class: 'read', readOnly: true },
  ruview_verify: { class: 'execute', readOnly: true },
  ruview_node_monitor: { class: 'hardware-read', readOnly: true, hardware: true },
  ruview_calibrate: { class: 'workspace-write', writesWorkspace: true, confirmField: 'confirm' },
  ruview_node_flash: { class: 'hardware-write', writesWorkspace: true, hardware: true, confirmField: 'confirm' },
  ruview_guidance: { class: 'read', readOnly: true },
  ruview_spaces_list: { class: 'external-read', readOnly: true, requiredGrant: 'credential-use', openWorld: true, usesCredentials: true, mayRefreshCredentials: true },
  ruview_memory_search: { class: 'read', readOnly: true },
  ruview_kernel_selftest: { class: 'read', readOnly: true },
  ruview_doctor: { class: 'read', readOnly: true },
  ruview_firmware_plan: { class: 'read', readOnly: true },
  ruview_firmware_ports: { class: 'hardware-read', readOnly: true, hardware: true },
  ruview_train_plan: { class: 'read', readOnly: true },
  ruview_train_gate: { class: 'read', readOnly: true },
  ruview_train: { class: 'workspace-write', writesWorkspace: true, confirmField: 'confirm' },
  ruview_devices_scan: { class: 'hardware-read', readOnly: true, requiredGrant: 'device-access' },
  ruview_esp32_capture: { class: 'network-read', readOnly: true, requiredGrant: 'device-access' },
  ruview_mmwave_read: { class: 'hardware-read', readOnly: true, requiredGrant: 'device-access' },
  ruview_lidar_read: { class: 'hardware-read', readOnly: true, requiredGrant: 'device-access', openWorld: true },
  ruview_host_list: { class: 'read', readOnly: true },
  ruview_host_run: { class: 'remote-read', readOnly: true, requiredGrant: 'remote-host', openWorld: true },
});

function typeMatches(value, type) {
  if (type === 'array') return Array.isArray(value);
  if (type === 'object') return value !== null && typeof value === 'object' && !Array.isArray(value);
  if (type === 'number') return typeof value === 'number' && Number.isFinite(value);
  return typeof value === type;
}

export function validateArguments(schema, value, path = '$') {
  const errors = [];
  if (!typeMatches(value, schema.type || 'object')) return [`${path} must be ${schema.type || 'object'}`];
  if (schema.type === 'object') {
    const properties = schema.properties || {};
    for (const key of schema.required || []) if (!(key in value)) errors.push(`${path}.${key} is required`);
    for (const [key, item] of Object.entries(value)) {
      if (!Object.hasOwn(properties, key)) {
        if (schema.additionalProperties !== true) errors.push(`${path}.${key} is not allowed`);
        continue;
      }
      errors.push(...validateArguments(properties[key], item, `${path}.${key}`));
    }
  }
  if (schema.type === 'array') {
    if (schema.maxItems !== undefined && value.length > schema.maxItems) errors.push(`${path} exceeds maxItems`);
    if (schema.items) value.forEach((item, index) => errors.push(...validateArguments(schema.items, item, `${path}[${index}]`)));
  }
  if (schema.enum && !schema.enum.includes(value)) errors.push(`${path} must be one of ${schema.enum.join(', ')}`);
  if (schema.type === 'string') {
    if (schema.minLength !== undefined && value.length < schema.minLength) errors.push(`${path} is too short`);
    if (schema.maxLength !== undefined && value.length > schema.maxLength) errors.push(`${path} is too long`);
    if (schema.pattern !== undefined && !new RegExp(schema.pattern).test(value)) errors.push(`${path} has an invalid format`);
  }
  if (schema.type === 'number') {
    if (schema.minimum !== undefined && value < schema.minimum) errors.push(`${path} is below minimum`);
    if (schema.maximum !== undefined && value > schema.maximum) errors.push(`${path} exceeds maximum`);
  }
  return errors;
}

export function authorizeTool(name, args, context = {}) {
  const policy = TOOL_POLICY[name] || { class: 'unknown', denied: true };
  if (policy.denied) return { ok: false, reason: 'policy_missing', policy };
  if (context.source !== 'mcp') return { ok: true, policy };
  const grants = new Set(context.grants || []);
  if (policy.requiredGrant && !grants.has(policy.requiredGrant)) {
    return { ok: false, reason: 'authority_denied', requiredGrant: policy.requiredGrant, policy };
  }
  if (policy.readOnly) return { ok: true, policy };
  if (policy.confirmField && args?.[policy.confirmField] !== true) {
    return { ok: false, reason: 'not_confirmed', policy };
  }
  if (!grants.has(policy.class)) return { ok: false, reason: 'authority_denied', requiredGrant: policy.class, policy };
  return { ok: true, policy };
}

export function mcpAnnotations(name) {
  const policy = TOOL_POLICY[name] || {};
  return {
    readOnlyHint: policy.readOnly === true && policy.mayRefreshCredentials !== true,
    destructiveHint: policy.writesWorkspace === true || policy.hardware === true,
    idempotentHint: policy.readOnly === true && policy.mayRefreshCredentials !== true,
    openWorldHint: policy.openWorld === true,
  };
}
