"use strict";
/**
 * Browser-specific entry point with IndexedDB support
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __exportStar = (this && this.__exportStar) || function(m, exports) {
    for (var p in m) if (p !== "default" && !Object.prototype.hasOwnProperty.call(exports, p)) __createBinding(exports, m, p);
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.BrowserDagManager = void 0;
exports.createBrowserDag = createBrowserDag;
__exportStar(require("./index"), exports);
// Re-export with browser-specific defaults
const index_1 = require("./index");
/**
 * Create a browser-optimized DAG with IndexedDB persistence
 */
async function createBrowserDag(name) {
    const storage = new index_1.DagStorage();
    const dag = new index_1.RuDag({ name, storage });
    await dag.init();
    return dag;
}
/**
 * Browser storage manager for DAGs
 */
class BrowserDagManager {
    constructor() {
        this.initialized = false;
        this.storage = new index_1.DagStorage();
    }
    async init() {
        if (this.initialized)
            return;
        await this.storage.init();
        this.initialized = true;
    }
    async createDag(name) {
        await this.init();
        const dag = new index_1.RuDag({ name, storage: this.storage });
        await dag.init();
        return dag;
    }
    async loadDag(id) {
        await this.init();
        return index_1.RuDag.load(id, this.storage);
    }
    async listDags() {
        await this.init();
        return this.storage.list();
    }
    async deleteDag(id) {
        await this.init();
        return this.storage.delete(id);
    }
    async clearAll() {
        await this.init();
        return this.storage.clear();
    }
    async getStats() {
        await this.init();
        return this.storage.stats();
    }
    close() {
        this.storage.close();
        this.initialized = false;
    }
}
exports.BrowserDagManager = BrowserDagManager;
//# sourceMappingURL=browser.js.map