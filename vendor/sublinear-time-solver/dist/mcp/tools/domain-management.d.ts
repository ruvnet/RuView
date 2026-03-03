/**
 * Domain Management MCP Tools
 * Provides CRUD operations for domain registry through MCP interface
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
import { DomainRegistry } from './domain-registry.js';
export declare class DomainManagementTools {
    private domainRegistry;
    constructor();
    getTools(): Tool[];
    handleToolCall(name: string, args: any): Promise<any>;
    private registerDomain;
    private listDomains;
    private getDomain;
    private updateDomain;
    private unregisterDomain;
    private enableDomain;
    private disableDomain;
    private getSystemStatus;
    getDomainRegistry(): DomainRegistry;
}
