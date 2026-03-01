// RuVector Graph Explorer - Client-side Application
class GraphExplorer {
    constructor() {
        this.nodes = [];
        this.links = [];
        this.simulation = null;
        this.svg = null;
        this.g = null;
        this.zoom = null;
        this.selectedNode = null;
        this.ws = null;
        this.apiUrl = window.location.origin;

        this.init();
    }

    async init() {
        this.setupUI();
        this.setupD3();
        this.setupWebSocket();
        this.setupEventListeners();
        await this.loadInitialData();
    }

    setupUI() {
        // Show loading overlay
        this.showLoading(true);

        // Update connection status
        this.updateConnectionStatus('connecting');
    }

    setupD3() {
        const container = d3.select('#graph-canvas');
        const width = container.node().getBoundingClientRect().width;
        const height = container.node().getBoundingClientRect().height;

        // Create SVG
        this.svg = container.append('svg')
            .attr('width', width)
            .attr('height', height)
            .style('background', 'transparent');

        // Create zoom behavior
        this.zoom = d3.zoom()
            .scaleExtent([0.1, 10])
            .on('zoom', (event) => {
                this.g.attr('transform', event.transform);
            });

        this.svg.call(this.zoom);

        // Create main group
        this.g = this.svg.append('g');

        // Create force simulation
        this.simulation = d3.forceSimulation()
            .force('link', d3.forceLink().id(d => d.id).distance(100))
            .force('charge', d3.forceManyBody().strength(-300))
            .force('center', d3.forceCenter(width / 2, height / 2))
            .force('collision', d3.forceCollide().radius(30));
    }

    setupWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}`;

        this.ws = new WebSocket(wsUrl);

        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.updateConnectionStatus('connected');
            this.showToast('Connected to server', 'success');
        };

        this.ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.handleWebSocketMessage(data);
        };

        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.updateConnectionStatus('error');
            this.showToast('Connection error', 'error');
        };

        this.ws.onclose = () => {
            console.log('WebSocket disconnected');
            this.updateConnectionStatus('disconnected');
            this.showToast('Disconnected from server', 'warning');

            // Attempt reconnection after 3 seconds
            setTimeout(() => this.setupWebSocket(), 3000);
        };
    }

    handleWebSocketMessage(data) {
        switch (data.type) {
            case 'update':
                this.handleGraphUpdate(data.payload);
                break;
            case 'node_added':
                this.handleNodeAdded(data.payload);
                break;
            case 'node_updated':
                this.handleNodeUpdated(data.payload);
                break;
            case 'similarity_result':
                this.handleSimilarityResult(data.payload);
                break;
            default:
                console.log('Unknown message type:', data.type);
        }
    }

    async loadInitialData() {
        try {
            const response = await fetch(`${this.apiUrl}/api/graph`);
            if (!response.ok) throw new Error('Failed to load graph data');

            const data = await response.json();
            this.updateGraph(data.nodes, data.links);
            this.showLoading(false);
            this.showToast('Graph loaded successfully', 'success');
        } catch (error) {
            console.error('Error loading data:', error);
            this.showLoading(false);
            this.showToast('Failed to load graph data', 'error');
        }
    }

    updateGraph(nodes, links) {
        this.nodes = nodes;
        this.links = links;

        this.updateStatistics();
        this.renderGraph();
    }

    renderGraph() {
        // Remove existing elements
        this.g.selectAll('.link').remove();
        this.g.selectAll('.node').remove();
        this.g.selectAll('.node-label').remove();

        // Create links
        const link = this.g.selectAll('.link')
            .data(this.links)
            .enter().append('line')
            .attr('class', 'link')
            .attr('stroke-width', d => Math.sqrt(d.similarity * 5) || 1);

        // Create nodes
        const node = this.g.selectAll('.node')
            .data(this.nodes)
            .enter().append('circle')
            .attr('class', 'node')
            .attr('r', 15)
            .attr('fill', d => this.getNodeColor(d))
            .call(this.drag(this.simulation))
            .on('click', (event, d) => this.handleNodeClick(event, d))
            .on('dblclick', (event, d) => this.handleNodeDoubleClick(event, d));

        // Create labels
        const label = this.g.selectAll('.node-label')
            .data(this.nodes)
            .enter().append('text')
            .attr('class', 'node-label')
            .attr('dy', -20)
            .text(d => d.label || d.id.substring(0, 8));

        // Update simulation
        this.simulation.nodes(this.nodes);
        this.simulation.force('link').links(this.links);

        this.simulation.on('tick', () => {
            link
                .attr('x1', d => d.source.x)
                .attr('y1', d => d.source.y)
                .attr('x2', d => d.target.x)
                .attr('y2', d => d.target.y);

            node
                .attr('cx', d => d.x)
                .attr('cy', d => d.y);

            label
                .attr('x', d => d.x)
                .attr('y', d => d.y);
        });

        this.simulation.alpha(1).restart();
    }

    getNodeColor(node) {
        // Color based on metadata or cluster
        if (node.metadata && node.metadata.category) {
            const categories = ['research', 'code', 'documentation', 'test'];
            const index = categories.indexOf(node.metadata.category);
            const colors = ['#667eea', '#f093fb', '#4caf50', '#ff9800'];
            return colors[index] || '#667eea';
        }
        return '#667eea';
    }

    drag(simulation) {
        function dragstarted(event) {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            event.subject.fx = event.subject.x;
            event.subject.fy = event.subject.y;
        }

        function dragged(event) {
            event.subject.fx = event.x;
            event.subject.fy = event.y;
        }

        function dragended(event) {
            if (!event.active) simulation.alphaTarget(0);
            event.subject.fx = null;
            event.subject.fy = null;
        }

        return d3.drag()
            .on('start', dragstarted)
            .on('drag', dragged)
            .on('end', dragended);
    }

    handleNodeClick(event, node) {
        event.stopPropagation();

        // Deselect previous node
        this.g.selectAll('.node').classed('selected', false);

        // Select new node
        this.selectedNode = node;
        d3.select(event.currentTarget).classed('selected', true);

        // Show metadata panel
        this.showMetadata(node);
        this.updateStatistics();
    }

    handleNodeDoubleClick(event, node) {
        event.stopPropagation();
        this.findSimilarNodes(node.id);
    }

    showMetadata(node) {
        const panel = document.getElementById('metadata-panel');
        const content = document.getElementById('metadata-content');

        let html = `
            <div class="metadata-item">
                <strong>ID:</strong>
                <div>${node.id}</div>
            </div>
        `;

        if (node.metadata) {
            for (const [key, value] of Object.entries(node.metadata)) {
                html += `
                    <div class="metadata-item">
                        <strong>${key}:</strong>
                        <div>${JSON.stringify(value, null, 2)}</div>
                    </div>
                `;
            }
        }

        content.innerHTML = html;
        panel.style.display = 'block';
    }

    async findSimilarNodes(nodeId) {
        if (!nodeId && this.selectedNode) {
            nodeId = this.selectedNode.id;
        }

        if (!nodeId) {
            this.showToast('Please select a node first', 'warning');
            return;
        }

        this.showLoading(true);

        try {
            const minSimilarity = parseFloat(document.getElementById('min-similarity').value);
            const response = await fetch(
                `${this.apiUrl}/api/similarity/${nodeId}?threshold=${minSimilarity}`
            );

            if (!response.ok) throw new Error('Failed to find similar nodes');

            const data = await response.json();
            this.highlightSimilarNodes(data.similar);
            this.showToast(`Found ${data.similar.length} similar nodes`, 'success');
        } catch (error) {
            console.error('Error finding similar nodes:', error);
            this.showToast('Failed to find similar nodes', 'error');
        } finally {
            this.showLoading(false);
        }
    }

    highlightSimilarNodes(similarNodes) {
        // Reset highlights
        this.g.selectAll('.node').classed('highlighted', false);
        this.g.selectAll('.link').classed('highlighted', false);

        const similarIds = new Set(similarNodes.map(n => n.id));

        // Highlight nodes
        this.g.selectAll('.node')
            .classed('highlighted', d => similarIds.has(d.id));

        // Highlight links
        this.g.selectAll('.link')
            .classed('highlighted', d =>
                similarIds.has(d.source.id) && similarIds.has(d.target.id)
            );
    }

    async searchNodes(query) {
        if (!query.trim()) {
            this.renderGraph();
            return;
        }

        try {
            const response = await fetch(
                `${this.apiUrl}/api/search?q=${encodeURIComponent(query)}`
            );

            if (!response.ok) throw new Error('Search failed');

            const data = await response.json();
            this.highlightSearchResults(data.results);
            this.showToast(`Found ${data.results.length} matches`, 'success');
        } catch (error) {
            console.error('Search error:', error);
            this.showToast('Search failed', 'error');
        }
    }

    highlightSearchResults(results) {
        const resultIds = new Set(results.map(r => r.id));

        this.g.selectAll('.node')
            .style('opacity', d => resultIds.has(d.id) ? 1 : 0.2);

        this.g.selectAll('.link')
            .style('opacity', d =>
                resultIds.has(d.source.id) || resultIds.has(d.target.id) ? 0.6 : 0.1
            );
    }

    updateStatistics() {
        document.getElementById('stat-nodes').textContent = this.nodes.length;
        document.getElementById('stat-edges').textContent = this.links.length;
        document.getElementById('stat-selected').textContent =
            this.selectedNode ? this.selectedNode.id.substring(0, 8) : 'None';
    }

    updateConnectionStatus(status) {
        const statusEl = document.getElementById('connection-status');
        const dot = statusEl.querySelector('.status-dot');
        const text = statusEl.querySelector('.status-text');

        const statusMap = {
            connecting: { text: 'Connecting...', class: '' },
            connected: { text: 'Connected', class: 'connected' },
            disconnected: { text: 'Disconnected', class: '' },
            error: { text: 'Error', class: '' }
        };

        const config = statusMap[status] || statusMap.disconnected;
        text.textContent = config.text;
        dot.className = `status-dot ${config.class}`;
    }

    showLoading(show) {
        const overlay = document.getElementById('loading-overlay');
        overlay.classList.toggle('hidden', !show);
    }

    showToast(message, type = 'info') {
        const container = document.getElementById('toast-container');
        const toast = document.createElement('div');
        toast.className = `toast ${type}`;
        toast.textContent = message;

        container.appendChild(toast);

        setTimeout(() => {
            toast.style.animation = 'slideIn 0.3s ease-out reverse';
            setTimeout(() => toast.remove(), 300);
        }, 3000);
    }

    async exportPNG() {
        try {
            const svgElement = this.svg.node();
            const canvas = document.createElement('canvas');
            const ctx = canvas.getContext('2d');

            const bbox = svgElement.getBBox();
            canvas.width = bbox.width + 40;
            canvas.height = bbox.height + 40;

            // Fill background
            ctx.fillStyle = '#1a1a2e';
            ctx.fillRect(0, 0, canvas.width, canvas.height);

            const svgString = new XMLSerializer().serializeToString(svgElement);
            const img = new Image();
            const blob = new Blob([svgString], { type: 'image/svg+xml' });
            const url = URL.createObjectURL(blob);

            img.onload = () => {
                ctx.drawImage(img, 20, 20);
                canvas.toBlob((blob) => {
                    const link = document.createElement('a');
                    link.download = `graph-${Date.now()}.png`;
                    link.href = URL.createObjectURL(blob);
                    link.click();
                    URL.revokeObjectURL(url);
                    this.showToast('Graph exported as PNG', 'success');
                });
            };

            img.src = url;
        } catch (error) {
            console.error('Export error:', error);
            this.showToast('Failed to export PNG', 'error');
        }
    }

    exportSVG() {
        try {
            const svgElement = this.svg.node();
            const svgString = new XMLSerializer().serializeToString(svgElement);
            const blob = new Blob([svgString], { type: 'image/svg+xml' });

            const link = document.createElement('a');
            link.download = `graph-${Date.now()}.svg`;
            link.href = URL.createObjectURL(blob);
            link.click();

            this.showToast('Graph exported as SVG', 'success');
        } catch (error) {
            console.error('Export error:', error);
            this.showToast('Failed to export SVG', 'error');
        }
    }

    resetView() {
        this.svg.transition()
            .duration(750)
            .call(this.zoom.transform, d3.zoomIdentity);
    }

    fitView() {
        const bounds = this.g.node().getBBox();
        const parent = this.svg.node().getBoundingClientRect();
        const fullWidth = parent.width;
        const fullHeight = parent.height;
        const width = bounds.width;
        const height = bounds.height;

        const midX = bounds.x + width / 2;
        const midY = bounds.y + height / 2;

        const scale = 0.85 / Math.max(width / fullWidth, height / fullHeight);
        const translate = [fullWidth / 2 - scale * midX, fullHeight / 2 - scale * midY];

        this.svg.transition()
            .duration(750)
            .call(this.zoom.transform, d3.zoomIdentity.translate(translate[0], translate[1]).scale(scale));
    }

    zoomIn() {
        this.svg.transition().call(this.zoom.scaleBy, 1.3);
    }

    zoomOut() {
        this.svg.transition().call(this.zoom.scaleBy, 0.7);
    }

    setupEventListeners() {
        // Search
        const searchInput = document.getElementById('node-search');
        let searchTimeout;
        searchInput.addEventListener('input', (e) => {
            clearTimeout(searchTimeout);
            searchTimeout = setTimeout(() => this.searchNodes(e.target.value), 300);
        });

        document.getElementById('clear-search').addEventListener('click', () => {
            searchInput.value = '';
            this.renderGraph();
        });

        // Filters
        const similaritySlider = document.getElementById('min-similarity');
        similaritySlider.addEventListener('input', (e) => {
            document.getElementById('similarity-value').textContent =
                parseFloat(e.target.value).toFixed(2);
        });

        document.getElementById('apply-filters').addEventListener('click', () => {
            this.loadInitialData();
        });

        // Metadata panel
        document.getElementById('find-similar').addEventListener('click', () => {
            this.findSimilarNodes();
        });

        document.getElementById('close-metadata').addEventListener('click', () => {
            document.getElementById('metadata-panel').style.display = 'none';
            this.selectedNode = null;
            this.g.selectAll('.node').classed('selected', false);
            this.updateStatistics();
        });

        // Export
        document.getElementById('export-png').addEventListener('click', () => this.exportPNG());
        document.getElementById('export-svg').addEventListener('click', () => this.exportSVG());

        // View controls
        document.getElementById('reset-view').addEventListener('click', () => this.resetView());
        document.getElementById('zoom-in').addEventListener('click', () => this.zoomIn());
        document.getElementById('zoom-out').addEventListener('click', () => this.zoomOut());
        document.getElementById('fit-view').addEventListener('click', () => this.fitView());

        // Window resize
        window.addEventListener('resize', () => {
            const container = d3.select('#graph-canvas');
            const width = container.node().getBoundingClientRect().width;
            const height = container.node().getBoundingClientRect().height;

            this.svg
                .attr('width', width)
                .attr('height', height);

            this.simulation
                .force('center', d3.forceCenter(width / 2, height / 2))
                .alpha(0.3)
                .restart();
        });
    }

    handleGraphUpdate(data) {
        this.updateGraph(data.nodes, data.links);
    }

    handleNodeAdded(node) {
        this.nodes.push(node);
        this.renderGraph();
        this.showToast('New node added', 'info');
    }

    handleNodeUpdated(node) {
        const index = this.nodes.findIndex(n => n.id === node.id);
        if (index !== -1) {
            this.nodes[index] = { ...this.nodes[index], ...node };
            this.renderGraph();
            this.showToast('Node updated', 'info');
        }
    }

    handleSimilarityResult(data) {
        this.highlightSimilarNodes(data.similar);
    }
}

// Initialize application when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    window.graphExplorer = new GraphExplorer();
});
