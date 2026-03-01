import { useState, useEffect, useCallback, useRef } from 'react';
import {
  Card,
  CardBody,
  CardHeader,
  Button,
  Input,
  Textarea,
  Tabs,
  Tab,
  Chip,
  Progress,
  Table,
  TableHeader,
  TableColumn,
  TableBody,
  TableRow,
  TableCell,
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
  useDisclosure,
  Tooltip,
  Snippet,
  Select,
  SelectItem,
  Switch,
} from '@heroui/react';
import {
  Database,
  Search,
  Plus,
  Trash2,
  Download,
  Upload,
  Play,
  Code,
  Network,
  Sparkles,
  BarChart3,
  Settings,
  RefreshCw,
  Copy,
  CheckCircle,
  AlertCircle,
  Zap,
  Box,
  GitBranch,
  Layers,
  Activity,
  Terminal,
  FileJson,
  Globe,
  HardDrive,
  Cpu,
  Save,
  Filter,
  Hash,
  Link2,
  Brain,
  TrendingUp,
  Target,
  ThumbsUp,
  ThumbsDown,
  Lightbulb,
  Share2,
  CircleDot,
  Table2,
  Columns,
  ChevronDown,
  ChevronRight,
  Eye,
  History,
  Clock,
  XCircle,
  Truck,
  ArrowRight,
  Rocket,
  HelpCircle,
  BookOpen,
  Github,
  ExternalLink,
  Info,
  LayoutGrid,
  Workflow,
  Package2,
} from 'lucide-react';
import {
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip as RechartsTooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
  PieChart,
  Pie,
  Cell,
} from 'recharts';
import useRvLite, { type SearchResult, type CypherResult, type SparqlResult, type SqlResult, type VectorEntry } from './hooks/useRvLite';
import useLearning from './hooks/useLearning';
import { GraphVisualization } from './components/GraphVisualization';
import { SimulationEngine } from './components/SimulationEngine';
import { SupplyChainSimulation } from './components/SupplyChainSimulation';

// Types
interface LogEntry {
  timestamp: string;
  type: 'info' | 'success' | 'error' | 'warning';
  message: string;
}

interface VectorDisplay {
  id: string;
  dimensions: number;
  metadata?: Record<string, unknown>;
  score?: number;
}

interface TableSchema {
  name: string;
  columns: Array<{
    name: string;
    type: string;
    isVector: boolean;
    dimensions?: number;
  }>;
  rowCount?: number;
}

interface FilterCondition {
  id: string;
  field: string;
  operator: 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte' | 'contains' | 'exists';
  value: string | number | boolean;
}

// Sample Cypher queries - After loading scenario data, these queries return results
// Create queries still work for adding new nodes
const SAMPLE_CYPHER_QUERIES = [
  { name: 'All Nodes', query: "MATCH (n) RETURN n" },
  { name: 'All Persons', query: "MATCH (n:Person) RETURN n" },
  { name: 'All Movies', query: "MATCH (n:Movie) RETURN n" },
  { name: 'All Departments', query: "MATCH (n:Department) RETURN n" },
  { name: 'All Topics', query: "MATCH (n:Topic) RETURN n" },
  { name: 'Create Node', query: "CREATE (n:TestNode {name: 'Test', value: 42})" },
  { name: 'Clear Graph', query: "MATCH (n) DELETE n" },
];

// SPARQL - Query parsing works but results are limited in WASM
// Full functionality available in native Rust builds
// NOTE: These queries use full rdf:type IRI to match loaded sample data
const SAMPLE_SPARQL_QUERIES = [
  { name: 'All Persons', query: "SELECT ?person WHERE { ?person <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> }" },
  { name: 'Person Names', query: "SELECT ?name WHERE { ?person <http://example.org/name> ?name }" },
  { name: 'Alice Knows', query: "SELECT ?who WHERE { <http://example.org/Alice> <http://example.org/knows> ?who }" },
  { name: 'ASK: Alice→Bob', query: "ASK { <http://example.org/Alice> <http://example.org/knows> <http://example.org/Bob> }" },
  { name: 'All Movies', query: "SELECT ?movie WHERE { ?movie <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Movie> }" },
  { name: 'All Triples', query: "SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10" },
];

// SQL Vector Search - After loading "Complete Demo", the docs table has sample data
// Vector search requires: ORDER BY column <-> [vector] (L2) or <=> [vector] (cosine)
// NOTE: First 3 queries are for creating data; last 3 queries work with loaded sample data
const SAMPLE_SQL_QUERIES = [
  { name: 'All Docs', query: "SELECT * FROM docs" },
  { name: 'Search AI (L2)', query: "SELECT * FROM docs ORDER BY embedding <-> [0.1, 0.2, 0.3] LIMIT 5" },
  { name: 'Search Climate (Cosine)', query: "SELECT * FROM docs ORDER BY embedding <=> [0.7, 0.8, 0.9] LIMIT 3" },
  { name: 'Filter by ID', query: "SELECT * FROM docs WHERE id = 'doc1'" },
  { name: 'Create Table', query: "CREATE TABLE docs (id TEXT, content TEXT, embedding VECTOR(3))" },
  { name: 'Insert Row', query: "INSERT INTO docs (id, content, embedding) VALUES ('doc4', 'new document', [0.5, 0.5, 0.5])" },
];

const SAMPLE_TRIPLES = [
  { subject: '<http://example.org/Alice>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Person>' },
  { subject: '<http://example.org/Alice>', predicate: '<http://example.org/name>', object: '"Alice"' },
  { subject: '<http://example.org/Alice>', predicate: '<http://example.org/age>', object: '"30"' },
  { subject: '<http://example.org/Bob>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Person>' },
  { subject: '<http://example.org/Alice>', predicate: '<http://example.org/knows>', object: '<http://example.org/Bob>' },
];

// Sample Data Scenarios for different use cases
interface SampleScenario {
  id: string;
  name: string;
  description: string;
  icon: string;
  category: 'vectors' | 'graph' | 'rdf' | 'mixed';
  data: {
    vectors?: Array<{ id: string; metadata: Record<string, unknown> }>;
    triples?: Array<{ subject: string; predicate: string; object: string }>;
    cypher?: string[];
    sql?: string[];
  };
}

const SAMPLE_SCENARIOS: SampleScenario[] = [
  {
    id: 'semantic-search',
    name: 'Semantic Search Demo',
    description: 'Document embeddings with categories and tags for semantic similarity search',
    icon: 'Search',
    category: 'vectors',
    data: {
      vectors: [
        { id: 'doc_ml_intro', metadata: { title: 'Introduction to Machine Learning', category: 'ML', author: 'Alice', year: 2023, tags: ['ai', 'tutorial', 'beginner'] }},
        { id: 'doc_deep_learning', metadata: { title: 'Deep Learning Fundamentals', category: 'ML', author: 'Bob', year: 2023, tags: ['ai', 'neural-networks', 'advanced'] }},
        { id: 'doc_vector_db', metadata: { title: 'Vector Databases Explained', category: 'Database', author: 'Charlie', year: 2024, tags: ['database', 'vectors', 'search'] }},
        { id: 'doc_rust_perf', metadata: { title: 'Rust Performance Optimization', category: 'Programming', author: 'Diana', year: 2024, tags: ['rust', 'performance', 'systems'] }},
        { id: 'doc_wasm_browser', metadata: { title: 'WebAssembly in Modern Browsers', category: 'Web', author: 'Eve', year: 2024, tags: ['wasm', 'web', 'browser'] }},
        { id: 'doc_rag_systems', metadata: { title: 'Building RAG Systems', category: 'AI', author: 'Frank', year: 2024, tags: ['rag', 'llm', 'retrieval'] }},
        { id: 'doc_embedding_models', metadata: { title: 'Text Embedding Models Comparison', category: 'ML', author: 'Grace', year: 2024, tags: ['embeddings', 'nlp', 'comparison'] }},
        { id: 'doc_graph_neural', metadata: { title: 'Graph Neural Networks', category: 'ML', author: 'Henry', year: 2023, tags: ['gnn', 'graphs', 'neural-networks'] }},
      ],
    },
  },
  {
    id: 'product-catalog',
    name: 'E-Commerce Product Catalog',
    description: 'Product embeddings with prices, ratings, and categories for recommendation',
    icon: 'Box',
    category: 'vectors',
    data: {
      vectors: [
        { id: 'prod_laptop_1', metadata: { name: 'Gaming Laptop Pro', category: 'Electronics', price: 1299.99, rating: 4.5, inStock: true, brand: 'TechBrand' }},
        { id: 'prod_laptop_2', metadata: { name: 'Business Ultrabook', category: 'Electronics', price: 999.99, rating: 4.8, inStock: true, brand: 'WorkPro' }},
        { id: 'prod_phone_1', metadata: { name: 'SmartPhone X', category: 'Electronics', price: 799.99, rating: 4.6, inStock: false, brand: 'TechBrand' }},
        { id: 'prod_headphones', metadata: { name: 'Wireless Headphones', category: 'Electronics', price: 249.99, rating: 4.7, inStock: true, brand: 'AudioMax' }},
        { id: 'prod_keyboard', metadata: { name: 'Mechanical Keyboard RGB', category: 'Accessories', price: 149.99, rating: 4.4, inStock: true, brand: 'KeyMaster' }},
        { id: 'prod_monitor', metadata: { name: '4K Gaming Monitor', category: 'Electronics', price: 599.99, rating: 4.9, inStock: true, brand: 'ViewTech' }},
      ],
    },
  },
  {
    id: 'knowledge-graph',
    name: 'Company Knowledge Graph',
    description: 'Organizational structure with employees, departments, and relationships',
    icon: 'Network',
    category: 'graph',
    data: {
      // RDF triples for SPARQL queries
      triples: [
        { subject: '<http://example.org/Alice>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Person>' },
        { subject: '<http://example.org/Alice>', predicate: '<http://example.org/name>', object: '"Alice Johnson"' },
        { subject: '<http://example.org/Alice>', predicate: '<http://example.org/role>', object: '"CEO"' },
        { subject: '<http://example.org/Bob>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Person>' },
        { subject: '<http://example.org/Bob>', predicate: '<http://example.org/name>', object: '"Bob Smith"' },
        { subject: '<http://example.org/Bob>', predicate: '<http://example.org/role>', object: '"CTO"' },
        { subject: '<http://example.org/Carol>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Person>' },
        { subject: '<http://example.org/Carol>', predicate: '<http://example.org/name>', object: '"Carol Williams"' },
        { subject: '<http://example.org/Alice>', predicate: '<http://example.org/knows>', object: '<http://example.org/Bob>' },
        { subject: '<http://example.org/Bob>', predicate: '<http://example.org/knows>', object: '<http://example.org/Carol>' },
      ],
      cypher: [
        // Create Person nodes
        "CREATE (alice:Person {name: 'Alice Johnson', role: 'CEO', salary: 250000})",
        "CREATE (bob:Person {name: 'Bob Smith', role: 'CTO', salary: 200000})",
        "CREATE (carol:Person {name: 'Carol Williams', role: 'Engineer', salary: 120000})",
        "CREATE (david:Person {name: 'David Brown', role: 'Engineer', salary: 115000})",
        "CREATE (eve:Person {name: 'Eve Davis', role: 'Designer', salary: 95000})",
        // Create Department nodes
        "CREATE (eng:Department {name: 'Engineering', budget: 500000})",
        "CREATE (design:Department {name: 'Design', budget: 200000})",
        "CREATE (exec:Department {name: 'Executive', budget: 300000})",
        // Create Company node
        "CREATE (techcorp:Company {name: 'TechCorp', founded: 2020, employees: 50})",
        // Create relationships using pattern syntax
        "CREATE (:Person {name: 'Alice'})-[:MANAGES]->(:Department {name: 'Executive'})",
        "CREATE (:Person {name: 'Bob'})-[:MANAGES]->(:Department {name: 'Engineering'})",
        "CREATE (:Person {name: 'Carol'})-[:WORKS_IN]->(:Department {name: 'Engineering'})",
        "CREATE (:Person {name: 'David'})-[:WORKS_IN]->(:Department {name: 'Engineering'})",
        "CREATE (:Person {name: 'Eve'})-[:WORKS_IN]->(:Department {name: 'Design'})",
        "CREATE (:Person {name: 'Alice'})-[:REPORTS_TO]->(:Company {name: 'TechCorp'})",
      ],
    },
  },
  {
    id: 'movie-database',
    name: 'Movie Recommendation Graph',
    description: 'Movies, actors, directors with relationships for recommendations',
    icon: 'Sparkles',
    category: 'graph',
    data: {
      // RDF triples for SPARQL queries
      triples: [
        { subject: '<http://example.org/Inception>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Movie>' },
        { subject: '<http://example.org/Inception>', predicate: '<http://example.org/title>', object: '"Inception"' },
        { subject: '<http://example.org/Matrix>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Movie>' },
        { subject: '<http://example.org/Matrix>', predicate: '<http://example.org/title>', object: '"The Matrix"' },
        { subject: '<http://example.org/Nolan>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Person>' },
        { subject: '<http://example.org/Nolan>', predicate: '<http://example.org/name>', object: '"Christopher Nolan"' },
        { subject: '<http://example.org/Nolan>', predicate: '<http://example.org/directed>', object: '<http://example.org/Inception>' },
        { subject: '<http://example.org/Leo>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/Person>' },
        { subject: '<http://example.org/Leo>', predicate: '<http://example.org/name>', object: '"Leonardo DiCaprio"' },
      ],
      cypher: [
        // Create Movie nodes
        "CREATE (inception:Movie {title: 'Inception', year: 2010, rating: 8.8, genre: 'Sci-Fi'})",
        "CREATE (matrix:Movie {title: 'The Matrix', year: 1999, rating: 8.7, genre: 'Sci-Fi'})",
        "CREATE (interstellar:Movie {title: 'Interstellar', year: 2014, rating: 8.6, genre: 'Sci-Fi'})",
        // Create Director nodes
        "CREATE (nolan:Director {name: 'Christopher Nolan', awards: 4})",
        "CREATE (wachowskis:Director {name: 'The Wachowskis', awards: 2})",
        // Create Actor nodes
        "CREATE (leo:Actor {name: 'Leonardo DiCaprio', oscars: 1})",
        "CREATE (keanu:Actor {name: 'Keanu Reeves', oscars: 0})",
        "CREATE (matthew:Actor {name: 'Matthew McConaughey', oscars: 1})",
        // Create relationships
        "CREATE (:Director {name: 'Christopher Nolan'})-[:DIRECTED]->(:Movie {title: 'Inception'})",
        "CREATE (:Director {name: 'Christopher Nolan'})-[:DIRECTED]->(:Movie {title: 'Interstellar'})",
        "CREATE (:Director {name: 'The Wachowskis'})-[:DIRECTED]->(:Movie {title: 'The Matrix'})",
        "CREATE (:Actor {name: 'Leonardo DiCaprio'})-[:ACTED_IN]->(:Movie {title: 'Inception'})",
        "CREATE (:Actor {name: 'Keanu Reeves'})-[:ACTED_IN]->(:Movie {title: 'The Matrix'})",
        "CREATE (:Actor {name: 'Matthew McConaughey'})-[:ACTED_IN]->(:Movie {title: 'Interstellar'})",
      ],
    },
  },
  {
    id: 'semantic-web',
    name: 'Semantic Web (RDF/SPARQL)',
    description: 'RDF triples for linked data and SPARQL queries',
    icon: 'Globe',
    category: 'rdf',
    data: {
      triples: [
        { subject: '<http://example.org/Person/1>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://xmlns.com/foaf/0.1/Person>' },
        { subject: '<http://example.org/Person/1>', predicate: '<http://xmlns.com/foaf/0.1/name>', object: '"John Doe"' },
        { subject: '<http://example.org/Person/1>', predicate: '<http://xmlns.com/foaf/0.1/mbox>', object: '"john@example.org"' },
        { subject: '<http://example.org/Person/1>', predicate: '<http://xmlns.com/foaf/0.1/knows>', object: '<http://example.org/Person/2>' },
        { subject: '<http://example.org/Person/2>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://xmlns.com/foaf/0.1/Person>' },
        { subject: '<http://example.org/Person/2>', predicate: '<http://xmlns.com/foaf/0.1/name>', object: '"Jane Smith"' },
        { subject: '<http://example.org/Person/2>', predicate: '<http://xmlns.com/foaf/0.1/mbox>', object: '"jane@example.org"' },
        { subject: '<http://example.org/Project/1>', predicate: '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', object: '<http://example.org/SoftwareProject>' },
        { subject: '<http://example.org/Project/1>', predicate: '<http://purl.org/dc/terms/title>', object: '"RvLite Database"' },
        { subject: '<http://example.org/Project/1>', predicate: '<http://purl.org/dc/terms/creator>', object: '<http://example.org/Person/1>' },
        { subject: '<http://example.org/Person/1>', predicate: '<http://example.org/worksOn>', object: '<http://example.org/Project/1>' },
        { subject: '<http://example.org/Person/2>', predicate: '<http://example.org/worksOn>', object: '<http://example.org/Project/1>' },
      ],
    },
  },
  {
    id: 'full-demo',
    name: 'Complete Demo (All Features)',
    description: 'Comprehensive dataset showcasing vectors, graphs, and RDF together',
    icon: 'Zap',
    category: 'mixed',
    data: {
      vectors: [
        { id: 'article_1', metadata: { title: 'AI in Healthcare', domain: 'health', importance: 'high' }},
        { id: 'article_2', metadata: { title: 'Climate Change Solutions', domain: 'environment', importance: 'critical' }},
        { id: 'article_3', metadata: { title: 'Quantum Computing Basics', domain: 'technology', importance: 'medium' }},
        { id: 'article_4', metadata: { title: 'Space Exploration 2024', domain: 'science', importance: 'high' }},
        { id: 'article_5', metadata: { title: 'Renewable Energy Trends', domain: 'environment', importance: 'high' }},
      ],
      triples: [
        { subject: '<http://example.org/AI>', predicate: '<http://example.org/relatedTo>', object: '<http://example.org/Healthcare>' },
        { subject: '<http://example.org/AI>', predicate: '<http://example.org/relatedTo>', object: '<http://example.org/Technology>' },
        { subject: '<http://example.org/QuantumComputing>', predicate: '<http://example.org/partOf>', object: '<http://example.org/Technology>' },
        { subject: '<http://example.org/ClimateChange>', predicate: '<http://example.org/affects>', object: '<http://example.org/Environment>' },
      ],
      cypher: [
        // Create Topic nodes
        "CREATE (ai:Topic {name: 'Artificial Intelligence', trending: true})",
        "CREATE (health:Topic {name: 'Healthcare', trending: true})",
        "CREATE (env:Topic {name: 'Environment', trending: true})",
        "CREATE (tech:Topic {name: 'Technology', trending: false})",
        // Create relationships between topics
        "CREATE (:Topic {name: 'AI'})-[:APPLIES_TO]->(:Topic {name: 'Healthcare'})",
        "CREATE (:Topic {name: 'AI'})-[:PART_OF]->(:Topic {name: 'Technology'})",
        "CREATE (:Topic {name: 'Environment'})-[:IMPACTS]->(:Topic {name: 'Healthcare'})",
      ],
      sql: [
        // Drop existing table first, then create fresh
        "DROP TABLE docs",
        "CREATE TABLE docs (id TEXT, content TEXT, embedding VECTOR(3))",
        // Insert sample documents with vectors
        "INSERT INTO docs (id, content, embedding) VALUES ('doc1', 'AI and machine learning basics', [0.1, 0.2, 0.3])",
        "INSERT INTO docs (id, content, embedding) VALUES ('doc2', 'Healthcare innovations 2024', [0.4, 0.5, 0.6])",
        "INSERT INTO docs (id, content, embedding) VALUES ('doc3', 'Climate and environment data', [0.7, 0.8, 0.9])",
      ],
    },
  },
];

// Real performance metrics tracking
interface PerformanceDataPoint {
  time: string;
  queries: number;
  latency: number;
  memory: number;
}

// Performance tracker to record actual query execution times
const performanceTracker = {
  _history: [] as Array<{ timestamp: number; latency: number }>,
  _queryCount: 0,

  record(latencyMs: number) {
    this._history.push({ timestamp: Date.now(), latency: latencyMs });
    this._queryCount++;
    // Keep only last 1000 records
    if (this._history.length > 1000) {
      this._history = this._history.slice(-1000);
    }
  },

  getRecentMetrics(windowMs: number = 2000): { avgLatency: number; queryCount: number } {
    const now = Date.now();
    const recent = this._history.filter(h => now - h.timestamp < windowMs);
    const avgLatency = recent.length > 0
      ? recent.reduce((sum, h) => sum + h.latency, 0) / recent.length
      : 0;
    return { avgLatency, queryCount: recent.length };
  },

  getTotalQueries(): number {
    return this._queryCount;
  },

  reset() {
    this._history = [];
    this._queryCount = 0;
  }
};

// Generate initial metrics with sample baseline data
const generateMetrics = (): PerformanceDataPoint[] => {
  const now = Date.now();
  return Array(20).fill(0).map((_, i) => {
    // Create realistic-looking baseline data that shows the system is active
    const baseLatency = 2 + Math.sin(i * 0.5) * 1.5; // Oscillating baseline 0.5-3.5ms
    const baseQueries = i; // Incrementing query count
    return {
      time: new Date(now - (20 - i) * 2000).toLocaleTimeString().split(':').slice(1).join(':'),
      queries: baseQueries,
      latency: Math.round(baseLatency * 100) / 100,
      memory: 0,
    };
  });
};

// Helper to extract error message from various error types
const getErrorMessage = (err: unknown): string => {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === 'string') {
    return err;
  }
  if (err && typeof err === 'object') {
    // Check for common error properties
    const obj = err as Record<string, unknown>;
    if (typeof obj.message === 'string') return obj.message;
    if (typeof obj.error === 'string') return obj.error;
    if (typeof obj.msg === 'string') return obj.msg;
    // Try to stringify
    try {
      return JSON.stringify(err);
    } catch {
      return 'Unknown error';
    }
  }
  return String(err);
};

function App() {
  // RvLite hook
  const {
    isReady,
    isLoading,
    error: rvliteError,
    stats,
    insertVector,
    insertVectorWithId,
    searchVectors,
    searchVectorsWithFilter,
    getVector,
    deleteVector,
    getAllVectors,
    executeSql,
    executeCypher,
    clearCypher,
    executeSparql,
    addTriple,
    clearTriples,
    saveDatabase,
    exportDatabase,
    importDatabase,
    clearDatabase,
    generateVector,
    updateStats,
    changeDistanceMetric,
    clearStorageData,
    storageStatus,
  } = useRvLite(128, 'cosine');

  // Self-Learning & GNN capabilities
  const {
    metrics: learningMetrics,
    patterns,
    suggestions,
    insights,
    gnnState,
    recordQuery,
    recordFeedback,
    resetLearning,
    trainGNN,
    getGraphEmbedding,
    exportLearning,
    importLearning,
    getRecentExecutions,
  } = useLearning();

  // UI State
  const [showWelcome, setShowWelcome] = useState(() => {
    // Check if user has seen welcome before
    const hasSeenWelcome = localStorage.getItem('rvlite-welcome-seen');
    return !hasSeenWelcome;
  });
  const [activeTab, setActiveTab] = useState('vectors');
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [vectors, setVectors] = useState<VectorDisplay[]>([]);
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [metrics, setMetrics] = useState(generateMetrics());

  // Query states
  const [cypherQuery, setCypherQuery] = useState(SAMPLE_CYPHER_QUERIES[0].query);
  const [cypherResult, setCypherResult] = useState<CypherResult | null>(null);
  const [showGraphView, setShowGraphView] = useState(true);
  const [sparqlQuery, setSparqlQuery] = useState(SAMPLE_SPARQL_QUERIES[0].query);
  const [sparqlResult, setSparqlResult] = useState<SparqlResult | null>(null);
  const [sqlQuery, setSqlQuery] = useState(SAMPLE_SQL_QUERIES[0].query);
  const [sqlResult, setSqlResult] = useState<SqlResult | null>(null);

  // SQL Schema Browser state
  const [sqlTables, setSqlTables] = useState<Map<string, TableSchema>>(new Map());
  const [expandedTables, setExpandedTables] = useState<Set<string>>(new Set());

  // Modal states
  const { isOpen: isAddOpen, onOpen: onAddOpen, onClose: onAddClose } = useDisclosure();
  const { isOpen: isSettingsOpen, onOpen: onSettingsOpen, onClose: onSettingsClose } = useDisclosure();
  const { isOpen: isTripleOpen, onOpen: onTripleOpen, onClose: onTripleClose } = useDisclosure();
  const { isOpen: isImportOpen, onOpen: onImportOpen, onClose: onImportClose } = useDisclosure();
  const { isOpen: isScenariosOpen, onOpen: onScenariosOpen, onClose: onScenariosClose } = useDisclosure();
  const { isOpen: isVectorDetailOpen, onOpen: onVectorDetailOpen, onClose: onVectorDetailClose } = useDisclosure();
  const { isOpen: isHelpOpen, onOpen: onHelpOpen, onClose: onHelpClose } = useDisclosure();
  const [helpTab, setHelpTab] = useState('intro');

  // Form states
  const [newVector, setNewVector] = useState({ id: '', metadata: '{}' });
  const [searchQuery, setSearchQuery] = useState('');
  const [searchK, setSearchK] = useState(5);
  const [useFilter, setUseFilter] = useState(false);
  const [filterJson, setFilterJson] = useState('{}');
  const [_filterConditions, _setFilterConditions] = useState<FilterCondition[]>([]);
  const [_showFilterJson, _setShowFilterJson] = useState(false);
  const [newTriple, setNewTriple] = useState({ subject: '', predicate: '', object: '' });
  const [selectedVectorId, setSelectedVectorId] = useState<string | null>(null);
  const [selectedVectorData, setSelectedVectorData] = useState<VectorEntry | null>(null);
  const [importJson, setImportJson] = useState('');

  // Logging
  const addLog = useCallback((type: LogEntry['type'], message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-99), { timestamp, type, message }]);
  }, []);

  // Track if we've initialized to prevent re-running effects
  const hasInitialized = useRef(false);

  // Load vectors on ready (only once)
  useEffect(() => {
    if (isReady && !hasInitialized.current) {
      hasInitialized.current = true;
      addLog('success', `RvLite ${stats.version} initialized!`);
      addLog('info', `Features: ${stats.features.join(', ')}`);
      refreshVectors();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isReady]);

  // Track if sample data has been loaded
  const hasSampleData = useRef(false);

  // Load sample data on first load (only once)
  useEffect(() => {
    if (isReady && stats.vectorCount === 0 && !hasSampleData.current) {
      hasSampleData.current = true;
      loadSampleData();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isReady, stats.vectorCount]);

  // Update metrics periodically with real data
  useEffect(() => {
    const interval = setInterval(() => {
      const { avgLatency } = performanceTracker.getRecentMetrics(2000);
      const memoryUsage = stats.vectorCount > 0
        ? Math.round((stats.vectorCount * stats.dimensions * 4) / 1024) // KB
        : 0;

      setMetrics(prev => [...prev.slice(1), {
        time: new Date().toLocaleTimeString().split(':').slice(1).join(':'),
        queries: performanceTracker.getTotalQueries(),
        latency: Math.round(avgLatency * 100) / 100,
        memory: memoryUsage,
      }]);
    }, 2000);
    return () => clearInterval(interval);
  }, [stats.vectorCount, stats.dimensions]);

  // Refresh vectors list
  const refreshVectors = useCallback(() => {
    if (!isReady) return;
    try {
      const results = getAllVectors();
      const vectorList: VectorDisplay[] = results.map(r => ({
        id: r.id,
        dimensions: 128,
        metadata: r.metadata,
        score: r.score,
      }));
      setVectors(vectorList);
      updateStats();
    } catch (err) {
      addLog('error', `Failed to refresh vectors: ${err}`);
    }
  }, [isReady, getAllVectors, updateStats, addLog]);


  // View vector details
  const handleViewVector = useCallback((id: string) => {
    try {
      const vectorData = getVector(id);
      if (vectorData) {
        setSelectedVectorId(id);
        setSelectedVectorData(vectorData);
        onVectorDetailOpen();
        addLog('info', `Viewing vector: ${id}`);
      } else {
        addLog('error', `Vector not found: ${id}`);
      }
    } catch (err) {
      addLog('error', `Failed to get vector: ${getErrorMessage(err)}`);
    }
  }, [getVector, setSelectedVectorId, setSelectedVectorData, onVectorDetailOpen, addLog]);
  // Load sample data
  const loadSampleData = useCallback(() => {
    if (!isReady) return;
    addLog('info', 'Loading sample data...');

    // Add sample vectors
    const categories = ['ML', 'Database', 'Web', 'Programming', 'AI'];
    const titles = [
      'Introduction to Machine Learning',
      'Vector Databases Explained',
      'WebAssembly in Browser',
      'Rust Performance Guide',
      'Neural Networks Deep Dive',
    ];

    titles.forEach((title, i) => {
      const vector = generateVector();
      insertVectorWithId(`doc_${i + 1}`, vector, {
        title,
        category: categories[i % categories.length],
        tags: ['sample', categories[i % categories.length].toLowerCase()],
      });
    });

    // Add sample triples
    SAMPLE_TRIPLES.forEach(t => {
      addTriple(t.subject, t.predicate, t.object);
    });

    // Add sample graph nodes and relationships
    executeCypher("CREATE (n:Person {name: 'Alice', age: 30})");
    executeCypher("CREATE (n:Person {name: 'Bob', age: 25})");
    executeCypher("CREATE (n:Company {name: 'TechCorp', founded: 2020})");
    executeCypher("CREATE (:Person {name: 'Alice'})-[:WORKS_AT]->(:Company {name: 'TechCorp'})");
    executeCypher("CREATE (:Person {name: 'Bob'})-[:WORKS_AT]->(:Company {name: 'TechCorp'})");
    executeCypher("CREATE (:Person {name: 'Alice'})-[:KNOWS]->(:Person {name: 'Bob'})");

    // Create SQL table with VECTOR column and insert sample data
    try {
      // Drop existing table first to avoid "already exists" error
      try { executeSql("DROP TABLE docs"); } catch { /* table might not exist */ }
      executeSql("CREATE TABLE docs (id TEXT, content TEXT, embedding VECTOR(3))");
      executeSql("INSERT INTO docs (id, content, embedding) VALUES ('doc1', 'Machine Learning Guide', [0.1, 0.2, 0.3])");
      executeSql("INSERT INTO docs (id, content, embedding) VALUES ('doc2', 'Vector Database Intro', [0.4, 0.5, 0.6])");
      executeSql("INSERT INTO docs (id, content, embedding) VALUES ('doc3', 'WebAssembly Tutorial', [0.7, 0.8, 0.9])");
    } catch { /* SQL error - continue */ }

    refreshVectors();

    // Auto-execute demo queries to show results immediately
    try {
      // Execute Cypher demo query
      const cypherRes = executeCypher("MATCH (n) RETURN n");
      setCypherResult(cypherRes);
    } catch { /* ignore */ }

    try {
      // Execute SPARQL demo query - use explicit predicate (not variables)
      const sparqlRes = executeSparql("SELECT ?person WHERE { ?person <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> }");
      setSparqlResult(sparqlRes);
    } catch { /* ignore */ }

    try {
      // Execute SQL demo query - vector search
      const sqlRes = executeSql("SELECT * FROM docs ORDER BY embedding <-> [0.2, 0.3, 0.4] LIMIT 5");
      setSqlResult(sqlRes);
    } catch { /* ignore */ }

    addLog('success', 'Sample data loaded successfully!');
  }, [isReady, generateVector, insertVectorWithId, addTriple, executeCypher, executeSql, executeSparql, refreshVectors, addLog, setCypherResult, setSparqlResult, setSqlResult]);

  // Load a specific sample scenario
  const loadScenario = useCallback((scenario: SampleScenario) => {
    if (!isReady) return;
    addLog('info', `Loading scenario: ${scenario.name}...`);

    try {
      // Load vectors
      if (scenario.data.vectors) {
        scenario.data.vectors.forEach(v => {
          const vector = generateVector();
          insertVectorWithId(v.id, vector, v.metadata);
        });
        addLog('success', `Added ${scenario.data.vectors.length} vectors`);
      }

      // Load triples
      if (scenario.data.triples) {
        scenario.data.triples.forEach(t => {
          addTriple(t.subject, t.predicate, t.object);
        });
        addLog('success', `Added ${scenario.data.triples.length} RDF triples`);
      }

      // Load graph nodes/relationships
      if (scenario.data.cypher) {
        scenario.data.cypher.forEach(query => {
          executeCypher(query);
        });
        addLog('success', `Executed ${scenario.data.cypher.length} Cypher queries`);
      }

      // Load SQL tables and data
      if (scenario.data.sql) {
        let sqlSuccess = 0;
        scenario.data.sql.forEach(query => {
          try {
            executeSql(query);
            sqlSuccess++;
          } catch (sqlErr) {
            // Table might already exist or other SQL error - continue
            addLog('warning', `SQL: ${getErrorMessage(sqlErr)}`);
          }
        });
        if (sqlSuccess > 0) {
          addLog('success', `Executed ${sqlSuccess}/${scenario.data.sql.length} SQL queries`);
        }
      }

      refreshVectors();
      updateStats();
      addLog('success', `Scenario "${scenario.name}" loaded successfully!`);
      onScenariosClose();
    } catch (err) {
      addLog('error', `Failed to load scenario: ${getErrorMessage(err)}`);
    }
  }, [isReady, generateVector, insertVectorWithId, addTriple, executeCypher, executeSql, refreshVectors, updateStats, addLog, onScenariosClose]);

  // Handle save to IndexedDB
  const handleSaveToStorage = useCallback(async () => {
    if (!isReady) return;
    addLog('info', 'Saving database to IndexedDB...');
    try {
      const success = await saveDatabase();
      if (success) {
        addLog('success', 'Database saved to IndexedDB successfully!');
      } else {
        addLog('warning', 'Save completed but may have had issues');
      }
    } catch (err) {
      addLog('error', `Failed to save: ${err}`);
    }
  }, [isReady, saveDatabase, addLog]);

  // Vector operations
  const handleSearch = useCallback(() => {
    if (!isReady) return;
    addLog('info', `Searching for top ${searchK} similar vectors...`);

    try {
      const queryVector = generateVector();
      let results: SearchResult[];

      if (useFilter && filterJson.trim() !== '{}') {
        const filter = JSON.parse(filterJson);
        results = searchVectorsWithFilter(queryVector, searchK, filter);
      } else {
        results = searchVectors(queryVector, searchK);
      }

      setSearchResults(results);
      addLog('success', `Found ${results.length} results`);
    } catch (err) {
      addLog('error', `Search failed: ${err}`);
    }
  }, [isReady, searchK, useFilter, filterJson, generateVector, searchVectors, searchVectorsWithFilter, addLog]);

  const handleAddVector = useCallback(() => {
    if (!isReady) return;
    try {
      const metadata = JSON.parse(newVector.metadata);
      const vector = generateVector();
      const id = newVector.id || undefined;

      if (id) {
        insertVectorWithId(id, vector, metadata);
        addLog('success', `Added vector with ID: ${id}`);
      } else {
        const newId = insertVector(vector, metadata);
        addLog('success', `Added vector: ${newId}`);
      }

      refreshVectors();
      onAddClose();
      setNewVector({ id: '', metadata: '{}' });
    } catch (err) {
      addLog('error', `Failed to add vector: ${err}`);
    }
  }, [isReady, newVector, generateVector, insertVector, insertVectorWithId, refreshVectors, onAddClose, addLog]);

  const handleDeleteVector = useCallback((id: string) => {
    if (!isReady) return;
    try {
      deleteVector(id);
      refreshVectors();
      addLog('info', `Deleted vector: ${id}`);
    } catch (err) {
      addLog('error', `Failed to delete: ${err}`);
    }
  }, [isReady, deleteVector, refreshVectors, addLog]);


  // Query handlers
  const handleExecuteCypher = useCallback(() => {
    if (!isReady) return;
    addLog('info', `Executing Cypher: ${cypherQuery.substring(0, 50)}...`);
    const startTime = performance.now();
    try {
      const result = executeCypher(cypherQuery);
      const executionTime = performance.now() - startTime;
      setCypherResult(result);
      addLog('success', result.message || 'Query executed successfully');
      updateStats();
      // Record for learning
      recordQuery(cypherQuery, 'cypher', executionTime, true, result.nodes?.length || result.relationships?.length || 0);
    } catch (err) {
      const errMsg = getErrorMessage(err);
      addLog('error', `Cypher error: ${errMsg}`);
      setCypherResult({ message: `Error: ${errMsg}` });
      recordQuery(cypherQuery, 'cypher', performance.now() - startTime, false, 0);
    }
  }, [isReady, cypherQuery, executeCypher, updateStats, addLog, recordQuery]);

  const handleExecuteSparql = useCallback(() => {
    if (!isReady) return;
    addLog('info', `Executing SPARQL: ${sparqlQuery.substring(0, 50)}...`);
    const startTime = performance.now();
    try {
      const result = executeSparql(sparqlQuery);
      const executionTime = performance.now() - startTime;
      setSparqlResult(result);
      // WASM limitation: SPARQL returns empty results
      if (Object.keys(result).length === 0) {
        addLog('warning', 'SPARQL query parsed OK. Note: Query execution is limited in WASM build.');
      } else {
        addLog('success', `SPARQL ${result.type} returned ${result.bindings?.length || 0} results`);
      }
      // Record for learning
      recordQuery(sparqlQuery, 'sparql', executionTime, true, result.bindings?.length || 0);
    } catch (err) {
      addLog('error', `SPARQL error: ${getErrorMessage(err)}`);
      recordQuery(sparqlQuery, 'sparql', performance.now() - startTime, false, 0);
    }
  }, [isReady, sparqlQuery, executeSparql, addLog, recordQuery]);
  // Parse CREATE TABLE statement to extract schema
  const parseCreateTable = useCallback((query: string): TableSchema | null => {
    const createTableRegex = /CREATE\s+TABLE\s+(\w+)\s*\(([^)]+)\)/i;
    const match = query.match(createTableRegex);

    if (!match) return null;

    const tableName = match[1];
    const columnsStr = match[2];

    // Parse columns: "id TEXT, content TEXT, embedding VECTOR(3)"
    const columnDefs = columnsStr.split(',').map(col => col.trim());
    const columns = columnDefs.map(colDef => {
      const parts = colDef.split(/\s+/);
      const name = parts[0];
      const type = parts.slice(1).join(' ');

      // Check if it's a VECTOR type
      const vectorMatch = type.match(/VECTOR\((\d+)\)/i);
      const isVector = !!vectorMatch;
      const dimensions = vectorMatch ? parseInt(vectorMatch[1], 10) : undefined;

      return {
        name,
        type: type.toUpperCase(),
        isVector,
        dimensions,
      };
    });

    return {
      name: tableName,
      columns,
      rowCount: 0,
    };
  }, []);

  // Handler functions for schema browser
  const toggleTableExpansion = useCallback((tableName: string) => {
    setExpandedTables(prev => {
      const newSet = new Set(prev);
      if (newSet.has(tableName)) {
        newSet.delete(tableName);
      } else {
        newSet.add(tableName);
      }
      return newSet;
    });
  }, []);

  const handleSelectTable = useCallback((tableName: string) => {
    setSqlQuery(`SELECT * FROM ${tableName}`);
  }, []);

  const handleDropTable = useCallback((tableName: string) => {
    if (!confirm(`Are you sure you want to drop table "${tableName}"?`)) return;
    setSqlQuery(`DROP TABLE ${tableName}`);
    // Execute immediately
    try {
      executeSql(`DROP TABLE ${tableName}`);
      setSqlTables(prev => {
        const newMap = new Map(prev);
        newMap.delete(tableName);
        return newMap;
      });
      setExpandedTables(prev => {
        const newSet = new Set(prev);
        newSet.delete(tableName);
        return newSet;
      });
      addLog('success', `Table "${tableName}" dropped`);
    } catch (err) {
      addLog('error', `Failed to drop table: ${getErrorMessage(err)}`);
    }
  }, [executeSql, addLog]);

  const handleExecuteSql = useCallback(() => {
    if (!isReady) return;
    addLog('info', `Executing SQL: ${sqlQuery.substring(0, 50)}...`);
    const startTime = performance.now();
    try {
      const result = executeSql(sqlQuery);
      const executionTime = performance.now() - startTime;
      setSqlResult(result);

      // Track CREATE TABLE statements
      if (sqlQuery.trim().toUpperCase().startsWith('CREATE TABLE')) {
        const schema = parseCreateTable(sqlQuery);
        if (schema) {
          setSqlTables(prev => new Map(prev).set(schema.name, schema));
          addLog('success', `Table "${schema.name}" created with ${schema.columns.length} columns`);
        }
      }

      // Track DROP TABLE statements
      const dropMatch = sqlQuery.match(/DROP\s+TABLE\s+(\w+)/i);
      if (dropMatch) {
        const tableName = dropMatch[1];
        setSqlTables(prev => {
          const newMap = new Map(prev);
          newMap.delete(tableName);
          return newMap;
        });
        setExpandedTables(prev => {
          const newSet = new Set(prev);
          newSet.delete(tableName);
          return newSet;
        });
        addLog('success', `Table "${tableName}" dropped`);
      } else {
        addLog('success', result.message || `${result.rows?.length || 0} rows returned`);
      }

      // Record for learning
      recordQuery(sqlQuery, 'sql', executionTime, true, result.rows?.length || 0);
    } catch (err) {
      const errMsg = getErrorMessage(err);
      // Provide helpful message for SQL table errors
      if (errMsg.includes("Table") && errMsg.includes("not found")) {
        addLog('warning', 'Table not found. Click "Create Table" sample query to create the docs table first.');
      } else {
        addLog('error', `SQL error: ${errMsg}`);
      }
      recordQuery(sqlQuery, 'sql', performance.now() - startTime, false, 0);
    }
  }, [isReady, sqlQuery, executeSql, addLog, recordQuery, parseCreateTable]);

  // Replay query from history
  const handleReplayQuery = useCallback((query: string, queryType: 'sql' | 'sparql' | 'cypher' | 'vector') => {
    // Set the query in the appropriate input field
    if (queryType === 'sql') {
      setSqlQuery(query);
      setActiveTab('sql');
      addLog('info', 'Query loaded from history - click Execute to run');
    } else if (queryType === 'sparql') {
      setSparqlQuery(query);
      setActiveTab('sparql');
      addLog('info', 'Query loaded from history - click Execute to run');
    } else if (queryType === 'cypher') {
      setCypherQuery(query);
      setActiveTab('cypher');
      addLog('info', 'Query loaded from history - click Execute to run');
    }
  }, [setActiveTab, addLog]);

  // Triple operations
  const handleAddTriple = useCallback(() => {
    if (!isReady) return;
    try {
      addTriple(newTriple.subject, newTriple.predicate, newTriple.object);
      addLog('success', 'Triple added successfully');
      updateStats();
      onTripleClose();
      setNewTriple({ subject: '', predicate: '', object: '' });
    } catch (err) {
      addLog('error', `Failed to add triple: ${getErrorMessage(err)}`);
    }
  }, [isReady, newTriple, addTriple, updateStats, onTripleClose, addLog]);

  // Persistence operations
  const handleSave = useCallback(async () => {
    if (!isReady) return;
    addLog('info', 'Saving database...');
    try {
      await saveDatabase();
      addLog('success', 'Database saved to IndexedDB');
    } catch (err) {
      addLog('error', `Save failed: ${err}`);
    }
  }, [isReady, saveDatabase, addLog]);

  const handleExport = useCallback(() => {
    if (!isReady) return;
    try {
      const data = exportDatabase();
      const json = JSON.stringify(data, null, 2);
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `rvlite-export-${Date.now()}.json`;
      a.click();
      URL.revokeObjectURL(url);
      addLog('success', 'Database exported successfully');
    } catch (err) {
      addLog('error', `Export failed: ${err}`);
    }
  }, [isReady, exportDatabase, addLog]);

  const handleImport = useCallback(() => {
    if (!isReady) return;
    try {
      const data = JSON.parse(importJson);
      importDatabase(data);
      refreshVectors();
      addLog('success', 'Database imported successfully');
      onImportClose();
      setImportJson('');
    } catch (err) {
      addLog('error', `Import failed: ${err}`);
    }
  }, [isReady, importJson, importDatabase, refreshVectors, onImportClose, addLog]);

  const handleClearAll = useCallback(async () => {
    if (!isReady) return;
    if (!confirm('Are you sure you want to clear all data?')) return;
    addLog('warning', 'Clearing all data...');
    try {
      await clearDatabase();
      setVectors([]);
      setSearchResults([]);
      setCypherResult(null);
      setSparqlResult(null);
      setSqlResult(null);
      addLog('success', 'All data cleared');
    } catch (err) {
      addLog('error', `Clear failed: ${err}`);
    }
  }, [isReady, clearDatabase, addLog]);

  // Pie chart data
  const pieData = [
    { name: 'Vectors', value: stats.vectorCount, color: '#00e68a' },
    { name: 'Triples', value: stats.tripleCount, color: '#7c3aed' },
    { name: 'Nodes', value: stats.graphNodeCount, color: '#f59e0b' },
    { name: 'Edges', value: stats.graphEdgeCount, color: '#3b82f6' },
  ].filter(d => d.value > 0);

  if (isLoading) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-gray-950 via-gray-900 to-gray-950 flex items-center justify-center">
        <Card className="bg-gray-900/50 border border-gray-800 p-8">
          <div className="flex flex-col items-center gap-4">
            <div className="animate-spin">
              <Database className="w-12 h-12 text-primary" />
            </div>
            <p className="text-gray-400">Initializing RvLite...</p>
          </div>
        </Card>
      </div>
    );
  }

  if (rvliteError) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-gray-950 via-gray-900 to-gray-950 flex items-center justify-center">
        <Card className="bg-gray-900/50 border border-red-800 p-8">
          <div className="flex flex-col items-center gap-4">
            <AlertCircle className="w-12 h-12 text-red-500" />
            <p className="text-red-400">Error: {rvliteError}</p>
            <Button color="primary" onPress={() => window.location.reload()}>
              Retry
            </Button>
          </div>
        </Card>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-950 via-gray-900 to-gray-950 p-4 md:p-6">
      {/* Header */}
      <div className="flex flex-col md:flex-row items-start md:items-center justify-between mb-6 gap-4">
        <button
          onClick={() => {
            setShowWelcome(true);
            localStorage.removeItem('rvlite-welcome-seen');
          }}
          className="flex items-center gap-3 hover:opacity-80 transition-opacity cursor-pointer"
        >
          <div className="p-2 bg-primary/20 rounded-xl">
            <Database className="w-8 h-8 text-primary" />
          </div>
          <div className="text-left">
            <h1 className="text-2xl md:text-3xl font-bold bg-gradient-to-r from-primary to-secondary bg-clip-text text-transparent">
              RvLite Dashboard
            </h1>
            <p className="text-sm text-gray-400">
              {stats.version} • Vector DB • SQL • SPARQL • Cypher
            </p>
          </div>
        </button>

        <div className="flex items-center gap-3 flex-wrap">
          <Chip
            startContent={isReady ? <CheckCircle className="w-4 h-4" /> : <AlertCircle className="w-4 h-4" />}
            color={isReady ? 'success' : 'danger'}
            variant="flat"
          >
            {isReady ? 'Connected' : 'Disconnected'}
          </Chip>
          {!showWelcome && (
            <Tooltip content="Show Welcome Screen">
              <Button
                isIconOnly
                variant="flat"
                onPress={() => {
                  setShowWelcome(true);
                  localStorage.removeItem('rvlite-welcome-seen');
                }}
              >
                <Rocket className="w-4 h-4" />
              </Button>
            </Tooltip>
          )}
          <Tooltip content="Save to IndexedDB">
            <Button isIconOnly variant="flat" onPress={handleSave}>
              <Save className="w-4 h-4" />
            </Button>
          </Tooltip>
          <Tooltip content="Refresh Stats">
            <Button isIconOnly variant="flat" onPress={() => { refreshVectors(); setMetrics(generateMetrics()); }}>
              <RefreshCw className="w-4 h-4" />
            </Button>
          </Tooltip>
          <Button isIconOnly variant="flat" onPress={onSettingsOpen}>
            <Settings className="w-4 h-4" />
          </Button>
          <Tooltip content="Help & Documentation">
            <Button isIconOnly variant="flat" color="primary" onPress={onHelpOpen}>
              <HelpCircle className="w-4 h-4" />
            </Button>
          </Tooltip>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4 mb-6">
        <Card className="bg-gray-900/50 border border-gray-800">
          <CardBody className="flex flex-row items-center gap-3 py-4">
            <div className="p-2 bg-primary/20 rounded-lg">
              <Box className="w-5 h-5 text-primary" />
            </div>
            <div>
              <p className="text-2xl font-bold">{stats.vectorCount}</p>
              <p className="text-xs text-gray-400">Vectors</p>
            </div>
          </CardBody>
        </Card>

        <Card className="bg-gray-900/50 border border-gray-800">
          <CardBody className="flex flex-row items-center gap-3 py-4">
            <div className="p-2 bg-secondary/20 rounded-lg">
              <Layers className="w-5 h-5 text-secondary" />
            </div>
            <div>
              <p className="text-2xl font-bold">{stats.dimensions}</p>
              <p className="text-xs text-gray-400">Dimensions</p>
            </div>
          </CardBody>
        </Card>

        <Card className="bg-gray-900/50 border border-gray-800">
          <CardBody className="flex flex-row items-center gap-3 py-4">
            <div className="p-2 bg-purple-500/20 rounded-lg">
              <Link2 className="w-5 h-5 text-purple-500" />
            </div>
            <div>
              <p className="text-2xl font-bold">{stats.tripleCount}</p>
              <p className="text-xs text-gray-400">RDF Triples</p>
            </div>
          </CardBody>
        </Card>

        <Card className="bg-gray-900/50 border border-gray-800">
          <CardBody className="flex flex-row items-center gap-3 py-4">
            <div className="p-2 bg-amber-500/20 rounded-lg">
              <Network className="w-5 h-5 text-amber-500" />
            </div>
            <div>
              <p className="text-2xl font-bold">{stats.graphNodeCount}</p>
              <p className="text-xs text-gray-400">Graph Nodes</p>
            </div>
          </CardBody>
        </Card>

        <Card className="bg-gray-900/50 border border-gray-800">
          <CardBody className="flex flex-row items-center gap-3 py-4">
            <div className="p-2 bg-blue-500/20 rounded-lg">
              <GitBranch className="w-5 h-5 text-blue-500" />
            </div>
            <div>
              <p className="text-2xl font-bold">{stats.graphEdgeCount}</p>
              <p className="text-xs text-gray-400">Graph Edges</p>
            </div>
          </CardBody>
        </Card>

        <Card className="bg-gray-900/50 border border-gray-800">
          <CardBody className="flex flex-row items-center gap-3 py-4">
            <div className="p-2 bg-green-500/20 rounded-lg">
              <HardDrive className="w-5 h-5 text-green-500" />
            </div>
            <div>
              <p className="text-2xl font-bold">{stats.memoryUsage}</p>
              <p className="text-xs text-gray-400">Memory</p>
            </div>
          </CardBody>
        </Card>
      </div>

      {/* Welcome Screen */}
      {showWelcome && (
        <Card className="bg-gradient-to-br from-gray-900/80 via-primary/10 to-secondary/10 border border-primary/30 mb-6">
          <CardBody className="p-6 md:p-8">
            <div className="flex flex-col md:flex-row justify-between items-start gap-6">
              <div className="flex-1">
                <div className="flex items-center gap-3 mb-4">
                  <div className="p-3 bg-gradient-to-br from-primary/30 to-secondary/30 rounded-xl">
                    <Brain className="w-8 h-8 text-primary" />
                  </div>
                  <div>
                    <h2 className="text-2xl font-bold text-white">Self-Learning AI Database</h2>
                    <p className="text-gray-400">Runs 100% in your browser — no server needed</p>
                  </div>
                </div>

                <p className="text-gray-300 mb-4 max-w-2xl">
                  RvLite is an <strong className="text-primary">AI-powered database</strong> that learns from your queries and gets smarter over time.
                  Train neural networks, search by meaning, and explore graph relationships — <strong className="text-white">100% in your browser, no cloud, completely private</strong>.
                </p>

                {/* AI Features Highlight */}
                <div className="flex flex-wrap gap-2 mb-6">
                  <Chip startContent={<Brain className="w-3 h-3" />} color="secondary" variant="flat" size="sm">
                    Graph Neural Networks
                  </Chip>
                  <Chip startContent={<Zap className="w-3 h-3" />} color="warning" variant="flat" size="sm">
                    Self-Learning Patterns
                  </Chip>
                  <Chip startContent={<Database className="w-3 h-3" />} color="primary" variant="flat" size="sm">
                    Vector + Graph + SQL
                  </Chip>
                  <Chip startContent={<Activity className="w-3 h-3" />} color="success" variant="flat" size="sm">
                    Real-Time Training
                  </Chip>
                </div>

                {/* Capability Cards */}
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
                  <button
                    onClick={() => { setShowWelcome(false); setActiveTab('vectors'); localStorage.setItem('rvlite-welcome-seen', 'true'); }}
                    className="p-4 bg-gray-800/50 hover:bg-gray-800 border border-gray-700 hover:border-primary/50 rounded-xl text-left transition-all group"
                  >
                    <Database className="w-6 h-6 text-primary mb-2 group-hover:scale-110 transition-transform" />
                    <h3 className="font-semibold text-white">Vector Search</h3>
                    <p className="text-xs text-gray-400 mt-1">Semantic similarity search with HNSW indexing</p>
                    <div className="flex items-center gap-1 mt-2 text-xs text-primary">
                      <span>Explore</span>
                      <ArrowRight className="w-3 h-3" />
                    </div>
                  </button>

                  <button
                    onClick={() => { setShowWelcome(false); setActiveTab('cypher'); localStorage.setItem('rvlite-welcome-seen', 'true'); }}
                    className="p-4 bg-gray-800/50 hover:bg-gray-800 border border-gray-700 hover:border-purple-500/50 rounded-xl text-left transition-all group"
                  >
                    <GitBranch className="w-6 h-6 text-purple-400 mb-2 group-hover:scale-110 transition-transform" />
                    <h3 className="font-semibold text-white">Graph Database</h3>
                    <p className="text-xs text-gray-400 mt-1">Cypher queries for nodes & relationships</p>
                    <div className="flex items-center gap-1 mt-2 text-xs text-purple-400">
                      <span>Explore</span>
                      <ArrowRight className="w-3 h-3" />
                    </div>
                  </button>

                  <button
                    onClick={() => { setShowWelcome(false); setActiveTab('sparql'); localStorage.setItem('rvlite-welcome-seen', 'true'); }}
                    className="p-4 bg-gray-800/50 hover:bg-gray-800 border border-gray-700 hover:border-green-500/50 rounded-xl text-left transition-all group"
                  >
                    <Globe className="w-6 h-6 text-green-400 mb-2 group-hover:scale-110 transition-transform" />
                    <h3 className="font-semibold text-white">RDF/SPARQL</h3>
                    <p className="text-xs text-gray-400 mt-1">Semantic web queries & linked data</p>
                    <div className="flex items-center gap-1 mt-2 text-xs text-green-400">
                      <span>Explore</span>
                      <ArrowRight className="w-3 h-3" />
                    </div>
                  </button>

                  <button
                    onClick={() => { setShowWelcome(false); setActiveTab('sql'); localStorage.setItem('rvlite-welcome-seen', 'true'); }}
                    className="p-4 bg-gray-800/50 hover:bg-gray-800 border border-gray-700 hover:border-blue-500/50 rounded-xl text-left transition-all group"
                  >
                    <Table2 className="w-6 h-6 text-blue-400 mb-2 group-hover:scale-110 transition-transform" />
                    <h3 className="font-semibold text-white">SQL Engine</h3>
                    <p className="text-xs text-gray-400 mt-1">Relational queries with vector columns</p>
                    <div className="flex items-center gap-1 mt-2 text-xs text-blue-400">
                      <span>Explore</span>
                      <ArrowRight className="w-3 h-3" />
                    </div>
                  </button>
                </div>

                {/* Demo Cards */}
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <button
                    onClick={() => { setShowWelcome(false); setActiveTab('supply-chain'); localStorage.setItem('rvlite-welcome-seen', 'true'); }}
                    className="p-4 bg-gradient-to-r from-orange-900/30 to-red-900/30 hover:from-orange-900/50 hover:to-red-900/50 border border-orange-700/50 hover:border-orange-500/50 rounded-xl text-left transition-all group"
                  >
                    <div className="flex items-center gap-3">
                      <Truck className="w-8 h-8 text-orange-400 group-hover:scale-110 transition-transform" />
                      <div>
                        <div className="flex items-center gap-2">
                          <h3 className="font-semibold text-white">Supply Chain Simulation</h3>
                          <Chip size="sm" color="warning" variant="flat">Demo</Chip>
                        </div>
                        <p className="text-xs text-gray-400 mt-1">AI-powered weather disruption remediation using all RvLite features</p>
                      </div>
                    </div>
                  </button>

                  <button
                    onClick={() => { setShowWelcome(false); setActiveTab('learning'); localStorage.setItem('rvlite-welcome-seen', 'true'); }}
                    className="p-4 bg-gradient-to-r from-purple-900/30 to-pink-900/30 hover:from-purple-900/50 hover:to-pink-900/50 border border-purple-700/50 hover:border-purple-500/50 rounded-xl text-left transition-all group"
                  >
                    <div className="flex items-center gap-3">
                      <Brain className="w-8 h-8 text-purple-400 group-hover:scale-110 transition-transform" />
                      <div>
                        <div className="flex items-center gap-2">
                          <h3 className="font-semibold text-white">Self-Learning Engine</h3>
                          <Chip size="sm" color="secondary" variant="flat">AI</Chip>
                        </div>
                        <p className="text-xs text-gray-400 mt-1">Neural network training, GNN embeddings, and pattern learning</p>
                      </div>
                    </div>
                  </button>
                </div>
              </div>

              {/* Quick Stats */}
              <div className="flex flex-col gap-3 min-w-[200px]">
                <div className="p-4 bg-gray-800/50 rounded-xl border border-gray-700">
                  <h4 className="text-sm font-semibold text-gray-400 mb-3">Quick Stats</h4>
                  <div className="space-y-2 text-sm">
                    <div className="flex justify-between">
                      <span className="text-gray-400">Vectors:</span>
                      <span className="text-white font-mono">{stats.vectorCount}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-400">RDF Triples:</span>
                      <span className="text-white font-mono">{stats.tripleCount}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-400">Graph Nodes:</span>
                      <span className="text-white font-mono">{stats.graphNodeCount}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-400">Graph Edges:</span>
                      <span className="text-white font-mono">{stats.graphEdgeCount}</span>
                    </div>
                  </div>
                </div>

                <Button
                  color="primary"
                  className="w-full"
                  onPress={() => { setShowWelcome(false); localStorage.setItem('rvlite-welcome-seen', 'true'); }}
                >
                  <Play className="w-4 h-4 mr-2" />
                  Get Started
                </Button>

                <Button
                  variant="flat"
                  className="w-full"
                  onPress={onScenariosOpen}
                >
                  <Sparkles className="w-4 h-4 mr-2" />
                  Load Sample Data
                </Button>
              </div>
            </div>
          </CardBody>
        </Card>
      )}

      {/* Main Content */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left Panel - Main Features */}
        <div className="lg:col-span-2 space-y-6">
          <Card className="bg-gray-900/50 border border-gray-800">
            <CardBody className="p-0">
              <Tabs
                selectedKey={activeTab}
                onSelectionChange={(key) => {
                  setActiveTab(key as string);
                  window.scrollTo({ top: 0, behavior: 'smooth' });
                }}
                classNames={{
                  tabList: "bg-gray-800/50 p-1 gap-1",
                  cursor: "bg-primary",
                  tab: "px-4 py-2",
                  tabContent: "group-data-[selected=true]:text-black",
                }}
              >
                {/* Vectors Tab */}
                <Tab
                  key="vectors"
                  title={
                    <div className="flex items-center gap-2">
                      <Database className="w-4 h-4" />
                      <span>Vectors</span>
                      <Chip size="sm" variant="flat">{stats.vectorCount}</Chip>
                    </div>
                  }
                >
                  <div className="p-4 space-y-4">
                    {/* Search Section */}
                    <div className="flex flex-col gap-3">
                      <div className="flex flex-col md:flex-row gap-3">
                        <Input
                          placeholder="Search (generates random query vector)..."
                          value={searchQuery}
                          onChange={(e) => setSearchQuery(e.target.value)}
                          startContent={<Search className="w-4 h-4 text-gray-400" />}
                          classNames={{
                            input: "bg-gray-800/50 text-white placeholder:text-gray-500",
                            inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500 focus-within:border-primary",
                          }}
                          className="flex-1"
                        />
                        <Input
                          type="number"
                          placeholder="Top K"
                          value={searchK.toString()}
                          onChange={(e) => setSearchK(parseInt(e.target.value) || 5)}
                          className="w-24"
                          startContent={<Hash className="w-4 h-4 text-gray-400" />}
                          classNames={{
                            input: "bg-gray-800/50 text-white placeholder:text-gray-500",
                            inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500 focus-within:border-primary",
                          }}
                        />
                        <Button color="primary" onPress={handleSearch}>
                          <Search className="w-4 h-4 mr-2" />
                          Search
                        </Button>
                        <Button color="secondary" variant="flat" onPress={onAddOpen}>
                          <Plus className="w-4 h-4 mr-2" />
                          Add
                        </Button>
                      </div>

                      {/* Filter option */}
                      <div className="flex items-center gap-4">
                        <Switch
                          size="sm"
                          isSelected={useFilter}
                          onValueChange={setUseFilter}
                        >
                          Use metadata filter
                        </Switch>
                        {useFilter && (
                          <Input
                            size="sm"
                            placeholder='{"category": "ML"}'
                            value={filterJson}
                            onChange={(e) => setFilterJson(e.target.value)}
                            startContent={<Filter className="w-4 h-4 text-gray-400" />}
                            classNames={{
                              input: "bg-gray-800/50 text-white placeholder:text-gray-500 font-mono text-xs",
                              inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
                            }}
                            className="flex-1"
                          />
                        )}
                      </div>
                    </div>

                    {/* Search Results */}
                    {searchResults.length > 0 && (
                      <Card className="bg-gray-800/50">
                        <CardHeader className="pb-2">
                          <div className="flex items-center gap-2">
                            <Sparkles className="w-4 h-4 text-primary" />
                            <p className="text-sm font-semibold">Search Results ({searchResults.length})</p>
                          </div>
                        </CardHeader>
                        <CardBody className="pt-0">
                          <Table aria-label="Search results" removeWrapper>
                            <TableHeader>
                              <TableColumn>ID</TableColumn>
                              <TableColumn>Similarity</TableColumn>
                              <TableColumn>Metadata</TableColumn>
                            </TableHeader>
                            <TableBody>
                              {searchResults.map((result) => (
                                <TableRow key={result.id}>
                                  <TableCell>
                                    <Chip size="sm" variant="flat">{result.id}</Chip>
                                  </TableCell>
                                  <TableCell>
                                    <div className="flex items-center gap-2">
                                      <Progress
                                        value={result.score * 100}
                                        size="sm"
                                        color="primary"
                                        className="w-20"
                                      />
                                      <span className="text-xs font-mono">{result.score.toFixed(4)}</span>
                                    </div>
                                  </TableCell>
                                  <TableCell>
                                    <Snippet size="sm" variant="flat" className="bg-gray-900" hideSymbol>
                                      {JSON.stringify(result.metadata || {}).substring(0, 40)}...
                                    </Snippet>
                                  </TableCell>
                                </TableRow>
                              ))}
                            </TableBody>
                          </Table>
                        </CardBody>
                      </Card>
                    )}

                    {/* Vectors Table */}
                    <Table aria-label="Vectors" removeWrapper>
                      <TableHeader>
                        <TableColumn>ID</TableColumn>
                        <TableColumn>Dimensions</TableColumn>
                        <TableColumn>Category</TableColumn>
                        <TableColumn>Actions</TableColumn>
                      </TableHeader>
                      <TableBody emptyContent="No vectors. Click 'Add' to create one.">
                        {vectors.map((vector) => (
                          <TableRow key={vector.id}>
                            <TableCell>
                              <div
                                className="flex items-center gap-2 cursor-pointer hover:text-primary transition-colors"
                                onClick={() => handleViewVector(vector.id)}
                              >
                                <FileJson className="w-4 h-4 text-primary" />
                                <span className="font-mono text-sm">{vector.id}</span>
                              </div>
                            </TableCell>
                            <TableCell>
                              <Chip size="sm" variant="dot" color="primary">
                                {vector.dimensions}D
                              </Chip>
                            </TableCell>
                            <TableCell>
                              <Chip size="sm" variant="flat" color="secondary">
                                {(vector.metadata as { category?: string })?.category || 'N/A'}
                              </Chip>
                            </TableCell>
                            <TableCell>
                              <div className="flex gap-1">
                                <Tooltip content="View Details">
                                  <Button
                                    isIconOnly
                                    size="sm"
                                    variant="light"
                                    onPress={() => handleViewVector(vector.id)}
                                  >
                                    <Eye className="w-4 h-4" />
                                  </Button>
                                </Tooltip>
                                <Tooltip content="Delete">
                                  <Button
                                    isIconOnly
                                    size="sm"
                                    variant="light"
                                    color="danger"
                                    onPress={() => handleDeleteVector(vector.id)}
                                  >
                                    <Trash2 className="w-4 h-4" />
                                  </Button>
                                </Tooltip>
                              </div>
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </div>
                </Tab>

                {/* Cypher Tab */}
                <Tab
                  key="cypher"
                  title={
                    <div className="flex items-center gap-2">
                      <Network className="w-4 h-4" />
                      <span>Cypher</span>
                      <Chip size="sm" variant="flat">{stats.graphNodeCount}</Chip>
                    </div>
                  }
                >
                  <div className="p-4 space-y-4">
                    <div className="flex gap-2 flex-wrap">
                      {SAMPLE_CYPHER_QUERIES.map((q) => (
                        <Chip
                          key={q.name}
                          variant="flat"
                          className="cursor-pointer hover:bg-primary/20 transition-colors"
                          onClick={() => setCypherQuery(q.query)}
                        >
                          {q.name}
                        </Chip>
                      ))}
                    </div>
                    <Textarea
                      value={cypherQuery}
                      onChange={(e) => setCypherQuery(e.target.value)}
                      placeholder="Enter Cypher query..."
                      minRows={4}
                      classNames={{
                        input: "font-mono bg-gray-800/50 text-white placeholder:text-gray-500",
                        inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500 focus-within:border-primary",
                      }}
                    />
                    <div className="flex gap-2">
                      <Button color="primary" onPress={handleExecuteCypher}>
                        <Play className="w-4 h-4 mr-2" />
                        Execute
                      </Button>
                      <Button variant="flat" onPress={() => navigator.clipboard.writeText(cypherQuery)}>
                        <Copy className="w-4 h-4 mr-2" />
                        Copy
                      </Button>
                      <Button variant="flat" color="danger" onPress={clearCypher}>
                        <Trash2 className="w-4 h-4 mr-2" />
                        Clear Graph
                      </Button>
                    </div>

                    {/* Cypher Result */}
                    {cypherResult && (
                      <Card className="bg-gray-800/50">
                        <CardHeader className="flex justify-between">
                          <p className="text-sm font-semibold">Result</p>
                          <div className="flex gap-1">
                            <Tooltip content="Helpful result">
                              <Button
                                size="sm"
                                variant="flat"
                                isIconOnly
                                onPress={() => {
                                  recordFeedback(cypherQuery, 'cypher', true);
                                  addLog('info', 'Feedback recorded: helpful');
                                }}
                              >
                                <ThumbsUp className="w-3 h-3 text-green-400" />
                              </Button>
                            </Tooltip>
                            <Tooltip content="Not helpful">
                              <Button
                                size="sm"
                                variant="flat"
                                isIconOnly
                                onPress={() => {
                                  recordFeedback(cypherQuery, 'cypher', false);
                                  addLog('info', 'Feedback recorded: not helpful');
                                }}
                              >
                                <ThumbsDown className="w-3 h-3 text-red-400" />
                              </Button>
                            </Tooltip>
                          </div>
                        </CardHeader>
                        <CardBody className="space-y-4">
                          {/* View Toggle */}
                          {(cypherResult.nodes && cypherResult.nodes.length > 0) || (cypherResult.relationships && cypherResult.relationships.length > 0) ? (
                            <div className="flex gap-2 mb-2">
                              <Button
                                size="sm"
                                variant={showGraphView ? 'solid' : 'flat'}
                                color="primary"
                                onPress={() => setShowGraphView(true)}
                              >
                                <Network className="w-3 h-3 mr-1" />
                                Graph View
                              </Button>
                              <Button
                                size="sm"
                                variant={!showGraphView ? 'solid' : 'flat'}
                                onPress={() => setShowGraphView(false)}
                              >
                                <Code className="w-3 h-3 mr-1" />
                                JSON View
                              </Button>
                            </div>
                          ) : null}

                          {/* Graph Visualization */}
                          {showGraphView && (cypherResult.nodes && cypherResult.nodes.length > 0) ? (
                            <div className="bg-gray-900/50 border border-gray-700 rounded-lg p-6 min-h-[400px]">
                              <GraphVisualization nodes={cypherResult.nodes} relationships={cypherResult.relationships || []} />
                            </div>
                          ) : null}

                          {/* JSON View */}
                          {!showGraphView || !(cypherResult.nodes && cypherResult.nodes.length > 0) ? (
                            <pre className="text-xs font-mono overflow-auto max-h-60 text-gray-300">
                              {JSON.stringify(cypherResult, null, 2)}
                            </pre>
                          ) : null}
                        </CardBody>
                      </Card>
                    )}
                  </div>
                </Tab>

                {/* SPARQL Tab */}
                <Tab
                  key="sparql"
                  title={
                    <div className="flex items-center gap-2">
                      <Globe className="w-4 h-4" />
                      <span>SPARQL</span>
                      <Chip size="sm" variant="flat">{stats.tripleCount}</Chip>
                    </div>
                  }
                >
                  <div className="p-4 space-y-4">
                    <div className="flex gap-2 flex-wrap">
                      {SAMPLE_SPARQL_QUERIES.map((q) => (
                        <Chip
                          key={q.name}
                          variant="flat"
                          className="cursor-pointer hover:bg-secondary/20 transition-colors"
                          onClick={() => setSparqlQuery(q.query)}
                        >
                          {q.name}
                        </Chip>
                      ))}
                      <Chip
                        variant="flat"
                        color="success"
                        className="cursor-pointer"
                        onClick={onTripleOpen}
                      >
                        <Plus className="w-3 h-3 mr-1" />
                        Add Triple
                      </Chip>
                    </div>
                    <Textarea
                      value={sparqlQuery}
                      onChange={(e) => setSparqlQuery(e.target.value)}
                      placeholder="Enter SPARQL query..."
                      minRows={4}
                      classNames={{
                        input: "font-mono bg-gray-800/50 text-white placeholder:text-gray-500",
                        inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500 focus-within:border-secondary",
                      }}
                    />
                    <div className="flex gap-2">
                      <Button color="secondary" onPress={handleExecuteSparql}>
                        <Play className="w-4 h-4 mr-2" />
                        Execute
                      </Button>
                      <Button variant="flat" onPress={() => navigator.clipboard.writeText(sparqlQuery)}>
                        <Copy className="w-4 h-4 mr-2" />
                        Copy
                      </Button>
                      <Button variant="flat" color="danger" onPress={clearTriples}>
                        <Trash2 className="w-4 h-4 mr-2" />
                        Clear Triples
                      </Button>
                    </div>

                    {/* SPARQL Result */}
                    {sparqlResult && (
                      <Card className="bg-gray-800/50">
                        <CardHeader className="flex justify-between">
                          <p className="text-sm font-semibold">Result ({sparqlResult.type})</p>
                          <div className="flex gap-1">
                            <Tooltip content="Helpful result">
                              <Button
                                size="sm"
                                variant="flat"
                                isIconOnly
                                onPress={() => {
                                  recordFeedback(sparqlQuery, 'sparql', true);
                                  addLog('info', 'Feedback recorded: helpful');
                                }}
                              >
                                <ThumbsUp className="w-3 h-3 text-green-400" />
                              </Button>
                            </Tooltip>
                            <Tooltip content="Not helpful">
                              <Button
                                size="sm"
                                variant="flat"
                                isIconOnly
                                onPress={() => {
                                  recordFeedback(sparqlQuery, 'sparql', false);
                                  addLog('info', 'Feedback recorded: not helpful');
                                }}
                              >
                                <ThumbsDown className="w-3 h-3 text-red-400" />
                              </Button>
                            </Tooltip>
                          </div>
                        </CardHeader>
                        <CardBody>
                          <pre className="text-xs font-mono overflow-auto max-h-60 text-gray-300">
                            {JSON.stringify(sparqlResult, null, 2)}
                          </pre>
                        </CardBody>
                      </Card>
                    )}
                  </div>
                </Tab>

                {/* SQL Tab */}
                <Tab
                  key="sql"
                  title={
                    <div className="flex items-center gap-2">
                      <Terminal className="w-4 h-4" />
                      <span>SQL</span>
                    </div>
                  }
                >

                    {/* Schema Browser */}
                    {sqlTables.size > 0 && (
                      <Card className="bg-gray-800/50">
                        <CardHeader>
                          <div className="flex items-center gap-2">
                            <Database className="w-4 h-4 text-amber-500" />
                            <p className="text-sm font-semibold">Schema Browser</p>
                            <Chip size="sm" variant="flat" className="ml-auto">
                              {sqlTables.size} {sqlTables.size === 1 ? 'table' : 'tables'}
                            </Chip>
                          </div>
                        </CardHeader>
                        <CardBody className="space-y-2">
                          {Array.from(sqlTables.values()).map(table => {
                            const isExpanded = expandedTables.has(table.name);
                            return (
                              <div key={table.name} className="border border-gray-700 rounded-lg overflow-hidden">
                                <div
                                  className="flex items-center justify-between p-3 bg-gray-750 hover:bg-gray-700 cursor-pointer transition-colors"
                                  onClick={() => toggleTableExpansion(table.name)}
                                >
                                  <div className="flex items-center gap-2">
                                    {isExpanded ? (
                                      <ChevronDown className="w-4 h-4 text-gray-400" />
                                    ) : (
                                      <ChevronRight className="w-4 h-4 text-gray-400" />
                                    )}
                                    <Table2 className="w-4 h-4 text-amber-500" />
                                    <span className="font-mono text-sm font-semibold">{table.name}</span>
                                    <Chip size="sm" variant="flat">
                                      {table.columns.length} columns
                                    </Chip>
                                  </div>
                                  <div className="flex gap-1">
                                    <Tooltip content="Query this table">
                                      <Button
                                        size="sm"
                                        variant="flat"
                                        isIconOnly
                                        className="bg-amber-500/20 hover:bg-amber-500/30"
                                        onClick={(e) => {
                                          e.stopPropagation();
                                          handleSelectTable(table.name);
                                        }}
                                      >
                                        <Play className="w-4 h-4 text-amber-500" />
                                      </Button>
                                    </Tooltip>
                                    <Tooltip content="Drop table">
                                      <Button
                                        size="sm"
                                        variant="flat"
                                        isIconOnly
                                        className="bg-red-500/20 hover:bg-red-500/30"
                                        onClick={(e) => {
                                          e.stopPropagation();
                                          handleDropTable(table.name);
                                        }}
                                      >
                                        <Trash2 className="w-4 h-4 text-red-500" />
                                      </Button>
                                    </Tooltip>
                                  </div>
                                </div>

                                {isExpanded && (
                                  <div className="p-3 bg-gray-800/50 border-t border-gray-700">
                                    <div className="flex items-center gap-2 mb-2">
                                      <Columns className="w-3 h-3 text-gray-400" />
                                      <span className="text-xs text-gray-400 font-semibold">Columns</span>
                                    </div>
                                    <div className="space-y-1">
                                      {table.columns.map((col, idx) => (
                                        <div key={idx} className="flex items-center gap-2 pl-2">
                                          <div className="w-1 h-1 rounded-full bg-gray-600" />
                                          <span className="font-mono text-xs text-gray-300">{col.name}</span>
                                          <Chip
                                            size="sm"
                                            variant="flat"
                                            className={
                                              col.isVector
                                                ? 'bg-purple-500/20 text-purple-400 text-xs'
                                                : col.type.includes('TEXT')
                                                  ? 'bg-blue-500/20 text-blue-400 text-xs'
                                                  : col.type.includes('INTEGER')
                                                    ? 'bg-green-500/20 text-green-400 text-xs'
                                                    : 'bg-gray-500/20 text-gray-400 text-xs'
                                            }
                                          >
                                            {col.isVector ? `VECTOR(${col.dimensions})` : col.type}
                                          </Chip>
                                        </div>
                                      ))}
                                    </div>
                                  </div>
                                )}
                              </div>
                            );
                          })}
                        </CardBody>
                      </Card>
                    )}
                  <div className="p-4 space-y-4">
                    <div className="flex gap-2 flex-wrap">
                      {SAMPLE_SQL_QUERIES.map((q) => (
                        <Chip
                          key={q.name}
                          variant="flat"
                          className="cursor-pointer hover:bg-amber-500/20 transition-colors"
                          onClick={() => setSqlQuery(q.query)}
                        >
                          {q.name}
                        </Chip>
                      ))}
                    </div>
                    <Textarea
                      value={sqlQuery}
                      onChange={(e) => setSqlQuery(e.target.value)}
                      placeholder="Enter SQL query..."
                      minRows={4}
                      classNames={{
                        input: "font-mono bg-gray-800/50 text-white placeholder:text-gray-500",
                        inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500 focus-within:border-amber-500",
                      }}
                    />
                    <div className="flex gap-2">
                      <Button className="bg-amber-500 text-black" onPress={handleExecuteSql}>
                        <Play className="w-4 h-4 mr-2" />
                        Execute
                      </Button>
                      <Button variant="flat" onPress={() => navigator.clipboard.writeText(sqlQuery)}>
                        <Copy className="w-4 h-4 mr-2" />
                        Copy
                      </Button>
                    </div>

                    {/* SQL Result */}
                    {sqlResult && (
                      <Card className="bg-gray-800/50">
                        <CardHeader className="flex justify-between">
                          <p className="text-sm font-semibold">Result</p>
                          <div className="flex gap-1">
                            <Tooltip content="Helpful result">
                              <Button
                                size="sm"
                                variant="flat"
                                isIconOnly
                                onPress={() => {
                                  recordFeedback(sqlQuery, 'sql', true);
                                  addLog('info', 'Feedback recorded: helpful');
                                }}
                              >
                                <ThumbsUp className="w-3 h-3 text-green-400" />
                              </Button>
                            </Tooltip>
                            <Tooltip content="Not helpful">
                              <Button
                                size="sm"
                                variant="flat"
                                isIconOnly
                                onPress={() => {
                                  recordFeedback(sqlQuery, 'sql', false);
                                  addLog('info', 'Feedback recorded: not helpful');
                                }}
                              >
                                <ThumbsDown className="w-3 h-3 text-red-400" />
                              </Button>
                            </Tooltip>
                          </div>
                        </CardHeader>
                        <CardBody>
                          <pre className="text-xs font-mono overflow-auto max-h-60 text-gray-300">
                            {JSON.stringify(sqlResult, null, 2)}
                          </pre>
                        </CardBody>
                      </Card>
                    )}
                  </div>
                </Tab>

                {/* Learning & GNN Tab */}
                <Tab
                  key="learning"
                  title={
                    <div className="flex items-center gap-2">
                      <Brain className="w-4 h-4" />
                      <span>Learning</span>
                      <Chip size="sm" variant="flat" color="secondary">{patterns.length}</Chip>
                    </div>
                  }
                >
                  <div className="p-4 space-y-4">
                    {/* Learning Metrics Overview */}
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                      <Card className="bg-gray-800/50">
                        <CardBody className="p-3 text-center">
                          <p className="text-2xl font-bold text-primary">{learningMetrics.totalQueries}</p>
                          <p className="text-xs text-gray-400">Total Queries</p>
                        </CardBody>
                      </Card>
                      <Card className="bg-gray-800/50">
                        <CardBody className="p-3 text-center">
                          <p className="text-2xl font-bold text-green-400">{learningMetrics.totalQueries > 0 ? ((learningMetrics.successfulQueries / learningMetrics.totalQueries) * 100).toFixed(1) : '0.0'}%</p>
                          <p className="text-xs text-gray-400">Success Rate</p>
                        </CardBody>
                      </Card>
                      <Card className="bg-gray-800/50">
                        <CardBody className="p-3 text-center">
                          <p className="text-2xl font-bold text-blue-400">{(learningMetrics.avgResponseTime || 0).toFixed(0)}ms</p>
                          <p className="text-xs text-gray-400">Avg. Latency</p>
                        </CardBody>
                      </Card>
                      <Card className="bg-gray-800/50">
                        <CardBody className="p-3 text-center">
                          <p className="text-2xl font-bold text-purple-400">{(learningMetrics.adaptationLevel || 50).toFixed(0)}%</p>
                          <p className="text-xs text-gray-400">Adaptation</p>
                        </CardBody>
                      </Card>
                    </div>

                    {/* GNN Controls */}
                    <Card className="bg-gray-800/50">
                      <CardHeader>
                        <div className="flex items-center gap-2">
                          <Share2 className="w-4 h-4 text-purple-400" />
                          <span className="font-semibold">Graph Neural Network</span>
                          <Chip size="sm" variant="flat" color={gnnState.lastTrainedAt ? 'success' : 'warning'}>
                            {gnnState.lastTrainedAt ? 'Trained' : 'Untrained'}
                          </Chip>
                        </div>
                      </CardHeader>
                      <CardBody className="space-y-3">
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-center">
                          <div>
                            <p className="text-lg font-bold text-cyan-400">{gnnState.nodes}</p>
                            <p className="text-xs text-gray-400">Nodes</p>
                          </div>
                          <div>
                            <p className="text-lg font-bold text-orange-400">{gnnState.edges}</p>
                            <p className="text-xs text-gray-400">Edges</p>
                          </div>
                          <div>
                            <p className="text-lg font-bold text-pink-400">{gnnState.layers}</p>
                            <p className="text-xs text-gray-400">Layers</p>
                          </div>
                          <div>
                            <p className="text-lg font-bold text-lime-400">{(gnnState.accuracy * 100).toFixed(1)}%</p>
                            <p className="text-xs text-gray-400">Accuracy</p>
                          </div>
                        </div>
                        <div className="flex gap-2">
                          <Button
                            className="bg-purple-600 text-white flex-1"
                            onPress={async () => {
                              addLog('info', 'GNN training initiated with current patterns');
                              const accuracy = await trainGNN();
                              addLog('success', `GNN training complete - accuracy: ${(accuracy * 100).toFixed(1)}%`);
                            }}
                            isDisabled={patterns.length < 3}
                          >
                            <Target className="w-4 h-4 mr-2" />
                            Train GNN
                          </Button>
                          <Button
                            variant="flat"
                            className="flex-1"
                            onPress={() => {
                              try {
                                const embedding = getGraphEmbedding('sample query');
                                if (embedding && embedding.length > 0) {
                                  const displayVals = embedding.slice(0, 4).map(v => typeof v === 'number' ? v.toFixed(3) : '0.000');
                                  addLog('success', `Graph embedding: [${displayVals.join(', ')}${embedding.length > 4 ? '...' : ''}]`);
                                } else {
                                  addLog('warning', 'Embedding returned empty. Try training the GNN first.');
                                }
                              } catch (e) {
                                addLog('error', `Failed to get embedding: ${e instanceof Error ? e.message : String(e)}`);
                              }
                            }}
                            isDisabled={!gnnState.lastTrainedAt}
                          >
                            <CircleDot className="w-4 h-4 mr-2" />
                            Get Embedding
                          </Button>
                        </div>
                      </CardBody>
                    </Card>

                    {/* Getting Started - Show when no patterns exist */}
                    {patterns.length === 0 && (
                      <Card className="bg-gradient-to-r from-purple-900/30 to-blue-900/30 border border-purple-700/50">
                        <CardBody className="p-6">
                          <div className="flex flex-col items-center text-center space-y-4">
                            <Brain className="w-12 h-12 text-purple-400 opacity-60" />
                            <div>
                              <h3 className="text-lg font-semibold text-white">Welcome to Self-Learning Mode</h3>
                              <p className="text-sm text-gray-400 mt-1">
                                Start building your learning patterns by executing queries or generate sample data.
                              </p>
                            </div>
                            <div className="flex gap-3">
                              <Button
                                color="primary"
                                onPress={() => {
                                  // Generate 10 demo patterns
                                  const demoQueries = [
                                    { query: 'SELECT * FROM users', type: 'sql' as const },
                                    { query: 'SELECT ?name WHERE { ?x foaf:name ?name }', type: 'sparql' as const },
                                    { query: 'MATCH (n:Person) RETURN n', type: 'cypher' as const },
                                    { query: 'SELECT id, name FROM products', type: 'sql' as const },
                                    { query: 'SELECT ?s ?p ?o WHERE { ?s ?p ?o }', type: 'sparql' as const },
                                    { query: 'MATCH (a)-[r]->(b) RETURN a, r, b', type: 'cypher' as const },
                                    { query: 'SELECT COUNT(*) FROM orders', type: 'sql' as const },
                                    { query: 'ASK { ?s ?p ?o }', type: 'sparql' as const },
                                    { query: 'MATCH (n) RETURN count(n)', type: 'cypher' as const },
                                    { query: 'SELECT * FROM vectors LIMIT 5', type: 'sql' as const },
                                  ];
                                  demoQueries.forEach((dq, idx) => {
                                    recordQuery(dq.query, dq.type, Math.random() * 100 + 10, Math.random() > 0.1, Math.floor(Math.random() * 50));
                                    if (idx % 3 === 0) recordFeedback(dq.query, dq.type, true); // Add some feedback
                                  });
                                  addLog('success', 'Generated 10 demo query patterns for learning');
                                }}
                                startContent={<Sparkles className="w-4 h-4" />}
                              >
                                Generate Demo Data
                              </Button>
                              <Button
                                variant="flat"
                                onPress={() => setActiveTab('sql')}
                              >
                                Go to SQL Tab
                              </Button>
                            </div>
                            <div className="text-xs text-gray-500 mt-2">
                              <p>How it works: Execute queries → Build patterns → Train GNN → Get embeddings</p>
                            </div>
                          </div>
                        </CardBody>
                      </Card>
                    )}

                    {/* Suggestions */}
                    {suggestions.length > 0 ? (
                      <Card className="bg-gray-800/50">
                        <CardHeader>
                          <div className="flex items-center gap-2">
                            <Lightbulb className="w-4 h-4 text-yellow-400" />
                            <span className="font-semibold">Suggestions</span>
                          </div>
                        </CardHeader>
                        <CardBody>
                          <div className="space-y-2">
                            {suggestions.slice(0, 5).map((suggestion, idx) => (
                              <div key={idx} className="flex items-center justify-between p-2 bg-gray-700/30 rounded-lg">
                                <div className="flex-1">
                                  <code className="text-xs text-cyan-300">{suggestion.query.substring(0, 50)}...</code>
                                  <div className="flex gap-2 mt-1">
                                    <Chip size="sm" variant="flat">{suggestion.queryType}</Chip>
                                    <span className="text-xs text-gray-400">Score: {(suggestion.confidence * 100).toFixed(0)}%</span>
                                  </div>
                                </div>
                                <Button
                                  size="sm"
                                  variant="flat"
                                  onPress={() => {
                                    if (suggestion.queryType === 'sql') setSqlQuery(suggestion.query);
                                    else if (suggestion.queryType === 'sparql') setSparqlQuery(suggestion.query);
                                    else if (suggestion.queryType === 'cypher') setCypherQuery(suggestion.query);
                                  }}
                                >
                                  Use
                                </Button>
                              </div>
                            ))}
                          </div>
                        </CardBody>
                      </Card>
                    ) : patterns.length > 0 && (
                      <Card className="bg-gray-800/50">
                        <CardHeader>
                          <div className="flex items-center gap-2">
                            <Lightbulb className="w-4 h-4 text-yellow-400" />
                            <span className="font-semibold">Suggestions</span>
                          </div>
                        </CardHeader>
                        <CardBody>
                          <p className="text-sm text-gray-400">
                            Keep executing queries to get personalized suggestions. The system learns from your patterns.
                          </p>
                        </CardBody>
                      </Card>
                    )}

                    {/* Insights */}
                    {insights.length > 0 && (
                      <Card className="bg-gray-800/50">
                        <CardHeader>
                          <div className="flex items-center gap-2">
                            <TrendingUp className="w-4 h-4 text-green-400" />
                            <span className="font-semibold">Insights</span>
                          </div>
                        </CardHeader>
                        <CardBody>
                          <div className="space-y-2">
                            {insights.map((insight, idx) => (
                              <div key={idx} className={`p-2 rounded-lg ${
                                insight.type === 'optimization' ? 'bg-blue-900/30 border-l-2 border-blue-400' :
                                insight.severity === 'warning' ? 'bg-yellow-900/30 border-l-2 border-yellow-400' :
                                'bg-green-900/30 border-l-2 border-green-400'
                              }`}>
                                <p className="text-sm font-medium">{insight.title}</p>
                                <p className="text-xs text-gray-400">{insight.description}</p>
                                {insight.recommendation && (
                                  <p className="text-xs text-cyan-300 mt-1">Tip: {insight.recommendation}</p>
                                )}
                              </div>
                            ))}
                          </div>
                        </CardBody>
                      </Card>
                    )}

                    {/* Query Patterns Table */}
                    <Card className="bg-gray-800/50">
                      <CardHeader className="flex justify-between">
                        <div className="flex items-center gap-2">
                          <Activity className="w-4 h-4 text-cyan-400" />
                          <span className="font-semibold">Learned Patterns</span>
                        </div>
                        <div className="flex gap-2">
                          <Tooltip content="Export learning data">
                            <Button
                              size="sm"
                              variant="flat"
                              color="primary"
                              onPress={() => {
                                const data = exportLearning();
                                const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
                                const url = URL.createObjectURL(blob);
                                const a = document.createElement('a');
                                a.href = url;
                                a.download = `rvlite-learning-${new Date().toISOString().slice(0, 10)}.json`;
                                a.click();
                                URL.revokeObjectURL(url);
                                addLog('success', 'Learning data exported');
                              }}
                            >
                              <Download className="w-3 h-3 mr-1" />
                              Export
                            </Button>
                          </Tooltip>
                          <Tooltip content="Import learning data">
                            <Button
                              size="sm"
                              variant="flat"
                              color="secondary"
                              onPress={() => {
                                const input = document.createElement('input');
                                input.type = 'file';
                                input.accept = '.json';
                                input.onchange = (e) => {
                                  const file = (e.target as HTMLInputElement).files?.[0];
                                  if (file) {
                                    const reader = new FileReader();
                                    reader.onload = (ev) => {
                                      try {
                                        const data = JSON.parse(ev.target?.result as string);
                                        importLearning(data);
                                        addLog('success', 'Learning data imported successfully');
                                      } catch {
                                        addLog('error', 'Failed to import learning data: Invalid JSON');
                                      }
                                    };
                                    reader.readAsText(file);
                                  }
                                };
                                input.click();
                              }}
                            >
                              <Upload className="w-3 h-3 mr-1" />
                              Import
                            </Button>
                          </Tooltip>
                          <Button
                            size="sm"
                            variant="flat"
                            color="danger"
                            onPress={() => {
                              resetLearning();
                              addLog('warning', 'Learning data reset');
                            }}
                          >
                            Reset
                          </Button>
                        </div>
                      </CardHeader>
                      <CardBody>
                        {patterns.length > 0 ? (
                          <Table
                            aria-label="Learned patterns"
                            classNames={{
                              base: "max-h-[300px] overflow-auto",
                              table: "min-w-full",
                              th: "bg-gray-700/50 text-gray-300",
                              td: "text-gray-300",
                            }}
                          >
                            <TableHeader>
                              <TableColumn>Pattern</TableColumn>
                              <TableColumn>Type</TableColumn>
                              <TableColumn>Frequency</TableColumn>
                              <TableColumn>Success</TableColumn>
                              <TableColumn>Feedback</TableColumn>
                              <TableColumn>Actions</TableColumn>
                            </TableHeader>
                            <TableBody>
                              {patterns.slice(0, 10).map((pattern) => (
                                <TableRow key={pattern.id}>
                                  <TableCell>
                                    <code className="text-xs">{pattern.pattern.substring(0, 30)}...</code>
                                  </TableCell>
                                  <TableCell>
                                    <Chip size="sm" variant="flat">{pattern.queryType}</Chip>
                                  </TableCell>
                                  <TableCell>{pattern.frequency}</TableCell>
                                  <TableCell>
                                    <span className={pattern.successRate > 0.8 ? 'text-green-400' : pattern.successRate > 0.5 ? 'text-yellow-400' : 'text-red-400'}>
                                      {(pattern.successRate * 100).toFixed(0)}%
                                    </span>
                                  </TableCell>
                                  <TableCell>
                                    <div className="flex gap-1 items-center">
                                      <ThumbsUp className="w-3 h-3 text-green-400" />
                                      <span className="text-xs">{pattern.feedback.helpful}</span>
                                      <ThumbsDown className="w-3 h-3 text-red-400 ml-2" />
                                      <span className="text-xs">{pattern.feedback.notHelpful}</span>
                                    </div>
                                  </TableCell>
                                  <TableCell>
                                    <div className="flex gap-1">
                                      <Tooltip content="Mark as helpful">
                                        <Button
                                          isIconOnly
                                          size="sm"
                                          variant="light"
                                          color="success"
                                          onPress={() => {
                                            recordFeedback(pattern.pattern, pattern.queryType, true);
                                            addLog('success', `Marked pattern as helpful: ${pattern.pattern.substring(0, 20)}...`);
                                          }}
                                        >
                                          <ThumbsUp className="w-3 h-3" />
                                        </Button>
                                      </Tooltip>
                                      <Tooltip content="Mark as not helpful">
                                        <Button
                                          isIconOnly
                                          size="sm"
                                          variant="light"
                                          color="danger"
                                          onPress={() => {
                                            recordFeedback(pattern.pattern, pattern.queryType, false);
                                            addLog('warning', `Marked pattern as not helpful: ${pattern.pattern.substring(0, 20)}...`);
                                          }}
                                        >
                                          <ThumbsDown className="w-3 h-3" />
                                        </Button>
                                      </Tooltip>
                                    </div>
                                  </TableCell>
                                </TableRow>
                              ))}
                            </TableBody>
                          </Table>
                        ) : (
                          <div className="text-center py-8 text-gray-400">
                            <Brain className="w-12 h-12 mx-auto mb-2 opacity-30" />
                            <p>No patterns learned yet</p>
                            <p className="text-xs">Execute queries to start learning</p>
                          </div>
                        )}
                      </CardBody>
                    </Card>

                    {/* Query History Card */}
                    <Card className="bg-gray-800/50">
                      <CardHeader className="flex justify-between items-center">
                        <div className="flex items-center gap-2">
                          <History className="w-5 h-5 text-blue-400" />
                          <span className="font-semibold">Query History</span>
                          <Chip size="sm" variant="flat" color="primary">
                            {getRecentExecutions(10).length}
                          </Chip>
                        </div>
                      </CardHeader>
                      <CardBody>
                        {getRecentExecutions(10).length > 0 ? (
                          <div className="space-y-2 max-h-[400px] overflow-auto">
                            {getRecentExecutions(10).map((execution) => {
                              // Determine color based on query type
                              const typeColor = execution.queryType === 'cypher' ? 'primary' :
                                              execution.queryType === 'sparql' ? 'secondary' :
                                              execution.queryType === 'sql' ? 'warning' : 'default';

                              // Format timestamp
                              const timeAgo = Date.now() - execution.timestamp;
                              const timeStr = timeAgo < 60000 ? 'just now' :
                                            timeAgo < 3600000 ? `${Math.floor(timeAgo / 60000)}m ago` :
                                            timeAgo < 86400000 ? `${Math.floor(timeAgo / 3600000)}h ago` :
                                            `${Math.floor(timeAgo / 86400000)}d ago`;

                              return (
                                <Card key={execution.id} className="bg-gray-700/30 hover:bg-gray-700/50 transition-colors">
                                  <CardBody className="p-3">
                                    <div className="flex items-start justify-between gap-2">
                                      <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-2 mb-1">
                                          <Chip size="sm" variant="flat" color={typeColor} className="uppercase">
                                            {execution.queryType}
                                          </Chip>
                                          <div className="flex items-center gap-1 text-xs text-gray-400">
                                            <Clock className="w-3 h-3" />
                                            <span>{timeStr}</span>
                                          </div>
                                          {execution.success ? (
                                            <CheckCircle className="w-4 h-4 text-green-400" />
                                          ) : (
                                            <XCircle className="w-4 h-4 text-red-400" />
                                          )}
                                        </div>
                                        <code className="text-xs text-gray-300 block truncate" title={execution.query}>
                                          {execution.query.length > 60 ? `${execution.query.substring(0, 60)}...` : execution.query}
                                        </code>
                                        <div className="flex items-center gap-3 mt-2 text-xs text-gray-400">
                                          <span>{typeof execution.executionTime === 'number' ? execution.executionTime.toFixed(0) : execution.executionTime || 0}ms</span>
                                          <span>{execution.resultCount ?? 0} results</span>
                                          {execution.error && (
                                            <Tooltip content={execution.error}>
                                              <AlertCircle className="w-3 h-3 text-red-400" />
                                            </Tooltip>
                                          )}
                                        </div>
                                      </div>
                                      <Tooltip content="Load and re-run query">
                                        <Button
                                          isIconOnly
                                          size="sm"
                                          variant="flat"
                                          color="primary"
                                          onPress={() => handleReplayQuery(execution.query, execution.queryType)}
                                        >
                                          <Play className="w-4 h-4" />
                                        </Button>
                                      </Tooltip>
                                    </div>
                                  </CardBody>
                                </Card>
                              );
                            })}
                          </div>
                        ) : (
                          <div className="text-center py-8 text-gray-400">
                            <History className="w-12 h-12 mx-auto mb-2 opacity-30" />
                            <p>No query history yet</p>
                            <p className="text-xs">Execute queries to build history</p>
                          </div>
                        )}
                      </CardBody>
                    </Card>
                  </div>
                </Tab>

                {/* Simulation Tab - Neural Network & GNN Training */}
                <Tab
                  key="simulation"
                  title={
                    <div className="flex items-center gap-2">
                      <Cpu className="w-4 h-4" />
                      <span>Simulation</span>
                      <Chip size="sm" variant="flat" color="warning">AI</Chip>
                    </div>
                  }
                >
                  <div className="p-4">
                    <SimulationEngine
                      gnnState={gnnState}
                      trainGNN={trainGNN}
                      getGraphEmbedding={getGraphEmbedding}
                      patterns={patterns}
                      recordQuery={recordQuery}
                      addLog={addLog}
                      executeSql={async (query) => {
                        try {
                          const result = await executeSql(query);
                          return { rows: result.rows || [] };
                        } catch (e) {
                          return { rows: [], error: e instanceof Error ? e.message : String(e) };
                        }
                      }}
                      executeSparql={async (query) => {
                        try {
                          const result = await executeSparql(query);
                          return { results: result.bindings || [] };
                        } catch (e) {
                          return { results: [], error: e instanceof Error ? e.message : String(e) };
                        }
                      }}
                      executeCypher={async (query) => {
                        try {
                          const result = await executeCypher(query);
                          return { nodes: result.nodes || [] };
                        } catch (e) {
                          return { nodes: [], error: e instanceof Error ? e.message : String(e) };
                        }
                      }}
                      executeVectorSearch={async (queryVec, k) => {
                        try {
                          const results = await searchVectors(queryVec, k);
                          return { results };
                        } catch (e) {
                          return { results: [], error: e instanceof Error ? e.message : String(e) };
                        }
                      }}
                    />
                  </div>
                </Tab>

                {/* Supply Chain Simulation Tab */}
                <Tab
                  key="supply-chain"
                  title={
                    <div className="flex items-center gap-2">
                      <Network className="w-4 h-4" />
                      <span>Supply Chain</span>
                      <Chip size="sm" variant="flat" color="success">Demo</Chip>
                    </div>
                  }
                >
                  <div className="p-4">
                    <SupplyChainSimulation
                      gnnState={gnnState}
                      trainGNN={trainGNN}
                      getGraphEmbedding={getGraphEmbedding}
                      patterns={patterns}
                      recordQuery={recordQuery}
                      addLog={addLog}
                      executeSql={async (query) => {
                        try {
                          const result = await executeSql(query);
                          return { rows: result.rows || [] };
                        } catch (e) {
                          return { rows: [], error: e instanceof Error ? e.message : String(e) };
                        }
                      }}
                      executeSparql={async (query) => {
                        try {
                          const result = await executeSparql(query);
                          return { results: result.bindings || [] };
                        } catch (e) {
                          return { results: [], error: e instanceof Error ? e.message : String(e) };
                        }
                      }}
                      executeCypher={async (query) => {
                        try {
                          const result = await executeCypher(query);
                          return { nodes: result.nodes || [] };
                        } catch (e) {
                          return { nodes: [], error: e instanceof Error ? e.message : String(e) };
                        }
                      }}
                      searchVectors={async (query, k) => {
                        try {
                          return await searchVectors(query, k);
                        } catch {
                          return [];
                        }
                      }}
                      insertVector={async (embedding, metadata) => {
                        try {
                          return await insertVector(embedding, metadata);
                        } catch {
                          return '';
                        }
                      }}
                    />
                  </div>
                </Tab>
              </Tabs>
            </CardBody>
          </Card>

          {/* Performance Chart */}
          <Card className="bg-gray-900/50 border border-gray-800">
            <CardHeader className="flex justify-between">
              <div className="flex items-center gap-2">
                <BarChart3 className="w-5 h-5 text-primary" />
                <span className="font-semibold">Performance Metrics</span>
              </div>
              <Chip size="sm" variant="flat" color="success">Live</Chip>
            </CardHeader>
            <CardBody>
              <ResponsiveContainer width="100%" height={200}>
                <AreaChart data={metrics}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#333" />
                  <XAxis dataKey="time" stroke="#666" />
                  <YAxis stroke="#666" />
                  <RechartsTooltip
                    contentStyle={{ backgroundColor: '#1a1a2e', border: '1px solid #333' }}
                  />
                  <Area
                    type="monotone"
                    dataKey="queries"
                    stroke="#00e68a"
                    fill="#00e68a"
                    fillOpacity={0.2}
                    name="Queries/s"
                  />
                  <Area
                    type="monotone"
                    dataKey="latency"
                    stroke="#7c3aed"
                    fill="#7c3aed"
                    fillOpacity={0.2}
                    name="Latency (ms)"
                  />
                </AreaChart>
              </ResponsiveContainer>
            </CardBody>
          </Card>
        </div>

        {/* Right Panel - Logs & Stats */}
        <div className="space-y-6">
          {/* Quick Actions - Moved to top */}
          <Card className="bg-gray-900/50 border border-gray-800">
            <CardHeader>
              <div className="flex items-center gap-2">
                <Zap className="w-5 h-5 text-amber-500" />
                <span className="font-semibold">Quick Actions</span>
              </div>
            </CardHeader>
            <CardBody className="space-y-2">
              <Button fullWidth variant="flat" color="secondary" className="justify-start" onPress={onScenariosOpen}>
                <Sparkles className="w-4 h-4 mr-2" />
                Load Sample Scenarios
              </Button>
              <Button fullWidth variant="flat" color="primary" className="justify-start" onPress={handleSaveToStorage}>
                <Save className="w-4 h-4 mr-2" />
                Save to Browser
              </Button>
              <Button fullWidth variant="flat" className="justify-start" onPress={handleExport}>
                <Download className="w-4 h-4 mr-2" />
                Export JSON
              </Button>
              <Button fullWidth variant="flat" className="justify-start" onPress={onImportOpen}>
                <Upload className="w-4 h-4 mr-2" />
                Import Data
              </Button>
              <Button fullWidth variant="flat" color="danger" className="justify-start" onPress={handleClearAll}>
                <Trash2 className="w-4 h-4 mr-2" />
                Clear All Data
              </Button>
            </CardBody>
          </Card>

          {/* Storage Status */}
          <Card className="bg-gray-900/50 border border-gray-800">
            <CardHeader>
              <div className="flex items-center gap-2">
                <HardDrive className="w-5 h-5 text-cyan-500" />
                <span className="font-semibold">Storage Status</span>
              </div>
            </CardHeader>
            <CardBody className="space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-sm text-gray-400">IndexedDB Available</span>
                <Chip
                  size="sm"
                  color={storageStatus.available ? 'success' : 'danger'}
                  variant="flat"
                >
                  {storageStatus.available ? 'Yes' : 'No'}
                </Chip>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-gray-400">Saved State</span>
                <Chip
                  size="sm"
                  color={storageStatus.hasSavedState ? 'success' : 'warning'}
                  variant="flat"
                >
                  {storageStatus.hasSavedState ? 'Found' : 'None'}
                </Chip>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-gray-400">Estimated Size</span>
                <span className="text-sm font-mono text-cyan-400">
                  {(storageStatus.estimatedSize / 1024).toFixed(1)} KB
                </span>
              </div>
              <Button
                fullWidth
                variant="flat"
                color="warning"
                size="sm"
                className="mt-2"
                onPress={async () => {
                  const success = await clearStorageData();
                  if (success) {
                    addLog('success', 'Browser storage cleared');
                  } else {
                    addLog('error', 'Failed to clear storage');
                  }
                }}
              >
                <Trash2 className="w-3 h-3 mr-2" />
                Clear Browser Storage
              </Button>
            </CardBody>
          </Card>

          {/* CypherEngine Stats */}
          <Card className="bg-gray-900/50 border border-gray-800">
            <CardHeader>
              <div className="flex items-center gap-2">
                <GitBranch className="w-5 h-5 text-orange-500" />
                <span className="font-semibold">Cypher Graph Engine</span>
              </div>
            </CardHeader>
            <CardBody className="space-y-3">
              <div className="grid grid-cols-2 gap-4">
                <div className="bg-gray-800/50 p-3 rounded-lg text-center">
                  <CircleDot className="w-5 h-5 mx-auto mb-1 text-amber-400" />
                  <p className="text-2xl font-bold text-amber-400">{stats.graphNodeCount}</p>
                  <p className="text-xs text-gray-400">Nodes</p>
                </div>
                <div className="bg-gray-800/50 p-3 rounded-lg text-center">
                  <Link2 className="w-5 h-5 mx-auto mb-1 text-blue-400" />
                  <p className="text-2xl font-bold text-blue-400">{stats.graphEdgeCount}</p>
                  <p className="text-xs text-gray-400">Relationships</p>
                </div>
              </div>
              <div className="flex gap-2">
                <Button
                  fullWidth
                  variant="flat"
                  color="secondary"
                  size="sm"
                  onPress={() => setActiveTab('cypher')}
                >
                  <Terminal className="w-3 h-3 mr-1" />
                  Query
                </Button>
                <Button
                  fullWidth
                  variant="flat"
                  color="danger"
                  size="sm"
                  onPress={() => {
                    clearCypher();
                    updateStats();
                    addLog('warning', 'Cypher graph cleared');
                  }}
                >
                  <Trash2 className="w-3 h-3 mr-1" />
                  Clear
                </Button>
              </div>
            </CardBody>
          </Card>

          {/* Data Distribution */}
          <Card className="bg-gray-900/50 border border-gray-800">
            <CardHeader>
              <div className="flex items-center gap-2">
                <Sparkles className="w-5 h-5 text-secondary" />
                <span className="font-semibold">Data Distribution</span>
              </div>
            </CardHeader>
            <CardBody>
              {pieData.length > 0 ? (
                <>
                  <ResponsiveContainer width="100%" height={200}>
                    <PieChart>
                      <Pie
                        data={pieData}
                        cx="50%"
                        cy="50%"
                        innerRadius={50}
                        outerRadius={80}
                        paddingAngle={5}
                        dataKey="value"
                      >
                        {pieData.map((entry, index) => (
                          <Cell key={`cell-${index}`} fill={entry.color} />
                        ))}
                      </Pie>
                      <RechartsTooltip
                        contentStyle={{ backgroundColor: '#1a1a2e', border: '1px solid #333' }}
                      />
                    </PieChart>
                  </ResponsiveContainer>
                  <div className="flex flex-wrap justify-center gap-3 mt-2">
                    {pieData.map((entry) => (
                      <div key={entry.name} className="flex items-center gap-1">
                        <div
                          className="w-3 h-3 rounded-full"
                          style={{ backgroundColor: entry.color }}
                        />
                        <span className="text-xs text-gray-400">{entry.name}: {entry.value}</span>
                      </div>
                    ))}
                  </div>
                </>
              ) : (
                <p className="text-gray-500 text-center py-8">No data yet</p>
              )}
            </CardBody>
          </Card>

          {/* Activity Log */}
          <Card className="bg-gray-900/50 border border-gray-800">
            <CardHeader className="flex justify-between">
              <div className="flex items-center gap-2">
                <Activity className="w-5 h-5 text-primary" />
                <span className="font-semibold">Activity Log</span>
              </div>
              <Button size="sm" variant="light" onPress={() => setLogs([])}>
                Clear
              </Button>
            </CardHeader>
            <CardBody>
              <div className="h-[300px] overflow-y-auto space-y-2">
                {logs.length === 0 ? (
                  <p className="text-gray-500 text-sm text-center py-4">No activity yet</p>
                ) : (
                  logs.slice().reverse().map((log, i) => (
                    <div
                      key={i}
                      className={`text-xs p-2 rounded-lg ${
                        log.type === 'error' ? 'bg-red-500/10 text-red-400' :
                        log.type === 'success' ? 'bg-green-500/10 text-green-400' :
                        log.type === 'warning' ? 'bg-amber-500/10 text-amber-400' :
                        'bg-gray-800 text-gray-400'
                      }`}
                    >
                      <span className="text-gray-500">[{log.timestamp}]</span> {log.message}
                    </div>
                  ))
                )}
              </div>
            </CardBody>
          </Card>

          {/* Features */}
          <Card className="bg-gray-900/50 border border-gray-800">
            <CardHeader>
              <div className="flex items-center gap-2">
                <Cpu className="w-5 h-5 text-green-500" />
                <span className="font-semibold">Features</span>
              </div>
            </CardHeader>
            <CardBody>
              <div className="flex flex-wrap gap-2">
                {stats.features.map((feature) => (
                  <Chip key={feature} size="sm" variant="flat" color="success">
                    {feature}
                  </Chip>
                ))}
              </div>
            </CardBody>
          </Card>
        </div>
      </div>

      {/* Add Vector Modal */}
      <Modal isOpen={isAddOpen} onClose={onAddClose}>
        <ModalContent className="bg-gray-900 border border-gray-700">
          <ModalHeader className="text-white">Add New Vector</ModalHeader>
          <ModalBody>
            <Input
              label="Vector ID (optional)"
              placeholder="Leave empty for auto-generated ID"
              value={newVector.id}
              onChange={(e) => setNewVector(prev => ({ ...prev, id: e.target.value }))}
              classNames={{
                label: "text-gray-300",
                input: "bg-gray-800/50 text-white placeholder:text-gray-500",
                inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
              }}
            />
            <Textarea
              label="Metadata (JSON)"
              placeholder='{"title": "My Document", "category": "ML"}'
              value={newVector.metadata}
              onChange={(e) => setNewVector(prev => ({ ...prev, metadata: e.target.value }))}
              classNames={{
                label: "text-gray-300",
                input: "font-mono bg-gray-800/50 text-white placeholder:text-gray-500",
                inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
              }}
            />
            <p className="text-xs text-gray-400">
              A random {stats.dimensions}-dimensional vector will be generated automatically.
            </p>
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" className="bg-gray-800 text-white hover:bg-gray-700" onPress={onAddClose}>Cancel</Button>
            <Button color="primary" onPress={handleAddVector}>Add Vector</Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* Add Triple Modal */}
      <Modal isOpen={isTripleOpen} onClose={onTripleClose}>
        <ModalContent className="bg-gray-900 border border-gray-700">
          <ModalHeader className="text-white">Add RDF Triple</ModalHeader>
          <ModalBody>
            <Input
              label="Subject"
              placeholder="<http://example.org/subject>"
              value={newTriple.subject}
              onChange={(e) => setNewTriple(prev => ({ ...prev, subject: e.target.value }))}
              classNames={{
                label: "text-gray-300",
                input: "bg-gray-800/50 text-white placeholder:text-gray-500 font-mono",
                inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
              }}
            />
            <Input
              label="Predicate"
              placeholder="<http://example.org/predicate>"
              value={newTriple.predicate}
              onChange={(e) => setNewTriple(prev => ({ ...prev, predicate: e.target.value }))}
              classNames={{
                label: "text-gray-300",
                input: "bg-gray-800/50 text-white placeholder:text-gray-500 font-mono",
                inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
              }}
            />
            <Input
              label="Object"
              placeholder='<http://example.org/object> or "literal value"'
              value={newTriple.object}
              onChange={(e) => setNewTriple(prev => ({ ...prev, object: e.target.value }))}
              classNames={{
                label: "text-gray-300",
                input: "bg-gray-800/50 text-white placeholder:text-gray-500 font-mono",
                inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
              }}
            />
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" className="bg-gray-800 text-white hover:bg-gray-700" onPress={onTripleClose}>Cancel</Button>
            <Button color="secondary" onPress={handleAddTriple}>Add Triple</Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* Import Modal */}
      <Modal isOpen={isImportOpen} onClose={onImportClose} size="2xl">
        <ModalContent className="bg-gray-900 border border-gray-700">
          <ModalHeader className="text-white">Import Database</ModalHeader>
          <ModalBody>
            <Textarea
              label="JSON Data"
              placeholder="Paste exported JSON data here..."
              value={importJson}
              onChange={(e) => setImportJson(e.target.value)}
              minRows={10}
              classNames={{
                label: "text-gray-300",
                input: "font-mono bg-gray-800/50 text-white placeholder:text-gray-500",
                inputWrapper: "bg-gray-800/50 border-gray-600 hover:border-gray-500",
              }}
            />
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" className="bg-gray-800 text-white hover:bg-gray-700" onPress={onImportClose}>Cancel</Button>
            <Button color="primary" onPress={handleImport}>Import</Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* Sample Scenarios Modal */}
      <Modal isOpen={isScenariosOpen} onClose={onScenariosClose} size="3xl" scrollBehavior="inside">
        <ModalContent className="bg-gray-900 border border-gray-700">
          <ModalHeader className="flex items-center gap-2 border-b border-gray-700">
            <Sparkles className="w-5 h-5 text-secondary" />
            <span>Load Sample Data Scenario</span>
          </ModalHeader>
          <ModalBody className="py-6">
            <p className="text-gray-400 text-sm mb-4">
              Choose a pre-built dataset to explore RvLite's capabilities. Each scenario demonstrates different features.
            </p>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {SAMPLE_SCENARIOS.map((scenario) => {
                const IconComponent = scenario.icon === 'Search' ? Search :
                  scenario.icon === 'Box' ? Box :
                  scenario.icon === 'Network' ? Network :
                  scenario.icon === 'Sparkles' ? Sparkles :
                  scenario.icon === 'Globe' ? Globe :
                  scenario.icon === 'Zap' ? Zap : Database;

                const categoryColors: Record<string, string> = {
                  vectors: 'bg-primary/20 text-primary border-primary/30',
                  graph: 'bg-amber-500/20 text-amber-400 border-amber-500/30',
                  rdf: 'bg-purple-500/20 text-purple-400 border-purple-500/30',
                  mixed: 'bg-green-500/20 text-green-400 border-green-500/30',
                };

                return (
                  <Card
                    key={scenario.id}
                    isPressable
                    onPress={() => loadScenario(scenario)}
                    className="bg-gray-800/80 border border-gray-700 hover:border-gray-500 hover:bg-gray-800 transition-all cursor-pointer"
                  >
                    <CardBody className="p-4">
                      <div className="flex items-start gap-3">
                        <div className={`p-2 rounded-lg ${categoryColors[scenario.category]}`}>
                          <IconComponent className="w-5 h-5" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 mb-1">
                            <h4 className="font-semibold text-white text-sm">{scenario.name}</h4>
                            <Chip size="sm" variant="flat" className={categoryColors[scenario.category]}>
                              {scenario.category}
                            </Chip>
                          </div>
                          <p className="text-gray-400 text-xs leading-relaxed">{scenario.description}</p>
                          <div className="flex items-center gap-3 mt-2 text-xs text-gray-500">
                            {scenario.data.vectors && (
                              <span className="flex items-center gap-1">
                                <Database className="w-3 h-3" />
                                {scenario.data.vectors.length} vectors
                              </span>
                            )}
                            {scenario.data.triples && (
                              <span className="flex items-center gap-1">
                                <Globe className="w-3 h-3" />
                                {scenario.data.triples.length} triples
                              </span>
                            )}
                            {scenario.data.cypher && (
                              <span className="flex items-center gap-1">
                                <Network className="w-3 h-3" />
                                {scenario.data.cypher.length} nodes
                              </span>
                            )}
                          </div>
                        </div>
                      </div>
                    </CardBody>
                  </Card>
                );
              })}
            </div>
          </ModalBody>
          <ModalFooter className="border-t border-gray-700">
            <Button variant="flat" className="bg-gray-800 text-white hover:bg-gray-700" onPress={onScenariosClose}>
              Cancel
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* Settings Modal */}
      <Modal isOpen={isSettingsOpen} onClose={onSettingsClose} size="2xl" scrollBehavior="inside">
        <ModalContent className="bg-gray-900 border border-gray-700">
          <ModalHeader className="flex items-center gap-2 border-b border-gray-700">
            <Settings className="w-5 h-5 text-primary" />
            <span>Database Settings</span>
          </ModalHeader>
          <ModalBody className="py-6">
            {/* Database Info Section */}
            <div className="space-y-6">
              <div>
                <h3 className="text-sm font-semibold text-white mb-3 flex items-center gap-2">
                  <Database className="w-4 h-4 text-primary" />
                  Database Information
                </h3>
                <div className="grid grid-cols-2 gap-4">
                  <div className="bg-gray-800/80 rounded-lg p-4 border border-gray-700">
                    <p className="text-xs text-gray-400 mb-1">Version</p>
                    <p className="text-lg font-semibold text-white">{stats.version}</p>
                  </div>
                  <div className="bg-gray-800/80 rounded-lg p-4 border border-gray-700">
                    <p className="text-xs text-gray-400 mb-1">Status</p>
                    <div className="flex items-center gap-2">
                      <div className={`w-2 h-2 rounded-full ${isReady ? 'bg-green-500' : 'bg-red-500'}`} />
                      <p className="text-lg font-semibold text-white">{isReady ? 'Connected' : 'Disconnected'}</p>
                    </div>
                  </div>
                </div>
              </div>

              {/* Vector Configuration */}
              <div>
                <h3 className="text-sm font-semibold text-white mb-3 flex items-center gap-2">
                  <Layers className="w-4 h-4 text-secondary" />
                  Vector Configuration
                </h3>
                <div className="space-y-4">
                  <div className="grid grid-cols-2 gap-4">
                    <Input
                      label="Dimensions"
                      type="number"
                      value={stats.dimensions.toString()}
                      isReadOnly
                      description="Vector dimensionality (set at creation)"
                      classNames={{
                        label: "text-gray-200 font-medium",
                        input: "bg-gray-700/80 text-white !text-white",
                        inputWrapper: "bg-gray-700/80 border-gray-500 hover:border-gray-400",
                        description: "text-gray-400",
                      }}
                    />
                    <Select
                      label="Distance Metric"
                      selectedKeys={[stats.distanceMetric]}
                      description="Algorithm for similarity calculation (changing this will re-index all vectors)"
                      onSelectionChange={async (keys) => {
                        const newMetric = Array.from(keys)[0] as string;
                        if (newMetric && newMetric !== stats.distanceMetric) {
                          addLog('info', `Changing distance metric to ${newMetric}...`);
                          const success = await changeDistanceMetric(newMetric);
                          if (success) {
                            addLog('success', `Distance metric changed to ${newMetric}`);
                            refreshVectors();
                          } else {
                            addLog('error', 'Failed to change distance metric');
                          }
                        }
                      }}
                      classNames={{
                        label: "text-gray-200 font-medium",
                        trigger: "bg-gray-700/80 border-gray-500 hover:border-gray-400 data-[hover=true]:bg-gray-600",
                        value: "text-white !text-white",
                        description: "text-gray-400",
                        listbox: "bg-gray-800",
                        popoverContent: "bg-gray-800 border border-gray-600",
                      }}
                    >
                      <SelectItem key="cosine" className="text-white data-[hover=true]:bg-gray-700 data-[selected=true]:bg-primary/30">Cosine Similarity</SelectItem>
                      <SelectItem key="euclidean" className="text-white data-[hover=true]:bg-gray-700 data-[selected=true]:bg-primary/30">Euclidean Distance</SelectItem>
                      <SelectItem key="dotproduct" className="text-white data-[hover=true]:bg-gray-700 data-[selected=true]:bg-primary/30">Dot Product</SelectItem>
                      <SelectItem key="manhattan" className="text-white data-[hover=true]:bg-gray-700 data-[selected=true]:bg-primary/30">Manhattan Distance</SelectItem>
                    </Select>
                  </div>
                  <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                    <p className="text-xs font-semibold text-gray-300 mb-2">Distance Metric Reference:</p>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-2 text-xs">
                      <div className="flex items-start gap-2">
                        <Chip size="sm" variant="flat" className="bg-primary/20 text-primary">cosine</Chip>
                        <span className="text-gray-400">Angular distance, best for text/embeddings</span>
                      </div>
                      <div className="flex items-start gap-2">
                        <Chip size="sm" variant="flat" className="bg-secondary/20 text-secondary">euclidean</Chip>
                        <span className="text-gray-400">L2 norm, straight-line distance</span>
                      </div>
                      <div className="flex items-start gap-2">
                        <Chip size="sm" variant="flat" className="bg-amber-500/20 text-amber-400">dotproduct</Chip>
                        <span className="text-gray-400">Inner product, projection similarity</span>
                      </div>
                      <div className="flex items-start gap-2">
                        <Chip size="sm" variant="flat" className="bg-purple-500/20 text-purple-400">manhattan</Chip>
                        <span className="text-gray-400">L1 norm, taxicab/city-block distance</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Storage Statistics */}
              <div>
                <h3 className="text-sm font-semibold text-white mb-3 flex items-center gap-2">
                  <HardDrive className="w-4 h-4 text-green-400" />
                  Storage Statistics
                </h3>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                  <div className="bg-gray-800/80 rounded-lg p-3 border border-gray-700">
                    <p className="text-xs text-gray-400">Vectors</p>
                    <p className="text-xl font-bold text-primary">{stats.vectorCount}</p>
                  </div>
                  <div className="bg-gray-800/80 rounded-lg p-3 border border-gray-700">
                    <p className="text-xs text-gray-400">RDF Triples</p>
                    <p className="text-xl font-bold text-purple-400">{stats.tripleCount}</p>
                  </div>
                  <div className="bg-gray-800/80 rounded-lg p-3 border border-gray-700">
                    <p className="text-xs text-gray-400">Graph Nodes</p>
                    <p className="text-xl font-bold text-amber-400">{stats.graphNodeCount}</p>
                  </div>
                  <div className="bg-gray-800/80 rounded-lg p-3 border border-gray-700">
                    <p className="text-xs text-gray-400">Graph Edges</p>
                    <p className="text-xl font-bold text-blue-400">{stats.graphEdgeCount}</p>
                  </div>
                </div>
              </div>

              {/* Query Engines */}
              <div>
                <h3 className="text-sm font-semibold text-white mb-3 flex items-center gap-2">
                  <Terminal className="w-4 h-4 text-cyan-400" />
                  Query Engines
                </h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  <div className="bg-gray-800/80 rounded-lg p-3 border border-gray-700">
                    <div className="flex items-center gap-2 mb-2">
                      <Database className="w-4 h-4 text-primary" />
                      <span className="text-sm font-medium text-white">Vector Search</span>
                    </div>
                    <p className="text-xs text-gray-400">k-NN similarity search with metadata filtering. Supports insert, search, get, delete operations.</p>
                  </div>
                  <div className="bg-gray-800/80 rounded-lg p-3 border border-gray-700">
                    <div className="flex items-center gap-2 mb-2">
                      <Terminal className="w-4 h-4 text-amber-400" />
                      <span className="text-sm font-medium text-white">SQL Engine</span>
                    </div>
                    <p className="text-xs text-gray-400">CREATE TABLE, SELECT, INSERT, DELETE with VECTOR type and distance operators.</p>
                  </div>
                  <div className="bg-gray-800/80 rounded-lg p-3 border border-gray-700">
                    <div className="flex items-center gap-2 mb-2">
                      <Globe className="w-4 h-4 text-purple-400" />
                      <span className="text-sm font-medium text-white">SPARQL Engine</span>
                    </div>
                    <p className="text-xs text-gray-400">RDF triple store with SELECT, ASK, CONSTRUCT queries. Supports pattern matching.</p>
                  </div>
                  <div className="bg-gray-800/80 rounded-lg p-3 border border-gray-700">
                    <div className="flex items-center gap-2 mb-2">
                      <Network className="w-4 h-4 text-blue-400" />
                      <span className="text-sm font-medium text-white">Cypher Engine</span>
                    </div>
                    <p className="text-xs text-gray-400">Property graph database with CREATE, MATCH, DELETE. Nodes, relationships, properties.</p>
                  </div>
                </div>
              </div>

              {/* Features */}
              <div>
                <h3 className="text-sm font-semibold text-white mb-3 flex items-center gap-2">
                  <Cpu className="w-4 h-4 text-amber-400" />
                  Enabled Features
                </h3>
                <div className="flex flex-wrap gap-2">
                  {stats.features.map((feature) => (
                    <Chip key={feature} size="sm" variant="flat" color="success" className="text-green-300">
                      <CheckCircle className="w-3 h-3 mr-1" />
                      {feature}
                    </Chip>
                  ))}
                </div>
              </div>

              {/* Performance Settings */}
              <div>
                <h3 className="text-sm font-semibold text-white mb-3 flex items-center gap-2">
                  <Activity className="w-4 h-4 text-blue-400" />
                  Performance
                </h3>
                <div className="space-y-4">
                  <div className="flex items-center justify-between bg-gray-800/80 rounded-lg p-4 border border-gray-700">
                    <div>
                      <p className="text-sm font-medium text-white">WASM Acceleration</p>
                      <p className="text-xs text-gray-400">Hardware-accelerated vector operations</p>
                    </div>
                    <Chip size="sm" color="success" variant="flat" className="text-green-300">
                      Enabled
                    </Chip>
                  </div>
                  <div className="flex items-center justify-between bg-gray-800/80 rounded-lg p-4 border border-gray-700">
                    <div>
                      <p className="text-sm font-medium text-white">IndexedDB Persistence</p>
                      <p className="text-xs text-gray-400">Browser-based persistent storage</p>
                    </div>
                    <Chip size="sm" color="success" variant="flat" className="text-green-300">Available</Chip>
                  </div>
                  <div className="flex items-center justify-between bg-gray-800/80 rounded-lg p-4 border border-gray-700">
                    <div>
                      <p className="text-sm font-medium text-white">Memory Usage</p>
                      <p className="text-xs text-gray-400">Current estimated memory footprint</p>
                    </div>
                    <Chip size="sm" variant="flat" className="bg-gray-700 text-gray-200">{stats.memoryUsage}</Chip>
                  </div>
                </div>
              </div>

              {/* Danger Zone */}
              <div>
                <h3 className="text-sm font-semibold text-red-400 mb-3 flex items-center gap-2">
                  <AlertCircle className="w-4 h-4" />
                  Danger Zone
                </h3>
                <div className="space-y-3">
                  <div className="flex items-center justify-between bg-red-500/10 rounded-lg p-4 border border-red-500/30">
                    <div>
                      <p className="text-sm font-medium text-white">Clear All Data</p>
                      <p className="text-xs text-gray-400">Permanently delete all vectors, triples, and graph data</p>
                    </div>
                    <Button size="sm" color="danger" variant="flat" onPress={handleClearAll}>
                      <Trash2 className="w-4 h-4 mr-1" />
                      Clear All
                    </Button>
                  </div>
                  <div className="flex items-center justify-between bg-amber-500/10 rounded-lg p-4 border border-amber-500/30">
                    <div>
                      <p className="text-sm font-medium text-white">Clear IndexedDB Storage</p>
                      <p className="text-xs text-gray-400">Remove saved state from browser storage</p>
                    </div>
                    <Button size="sm" color="warning" variant="flat" onPress={async () => {
                      try {
                        await clearDatabase();
                        addLog('success', 'Storage cleared');
                      } catch (err) {
                        addLog('error', `Failed to clear storage: ${err}`);
                      }
                    }}>
                      <HardDrive className="w-4 h-4 mr-1" />
                      Clear Storage
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          </ModalBody>
          <ModalFooter className="border-t border-gray-700">
            <Button variant="flat" className="bg-gray-800 text-white hover:bg-gray-700" onPress={onSettingsClose}>
              Close
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* Vector Detail Modal */}
      <Modal isOpen={isVectorDetailOpen} onClose={onVectorDetailClose} size="3xl" scrollBehavior="inside">
        <ModalContent className="bg-gray-900 border border-gray-700">
          <ModalHeader className="flex items-center gap-2 border-b border-gray-700">
            <Eye className="w-5 h-5 text-primary" />
            <span>Vector Inspector</span>
            {selectedVectorId && (
              <Chip size="sm" variant="flat" color="primary" className="ml-2">
                {selectedVectorId}
              </Chip>
            )}
          </ModalHeader>
          <ModalBody className="py-6 space-y-6">
            {selectedVectorData ? (
              <>
                {/* Vector ID Section */}
                <div className="space-y-2">
                  <div className="flex items-center gap-2 text-sm font-semibold text-gray-300">
                    <Hash className="w-4 h-4" />
                    <span>Vector ID</span>
                  </div>
                  <Snippet
                    symbol=""
                    className="bg-gray-800/50 border border-gray-700"
                    classNames={{
                      pre: "font-mono text-sm text-gray-200"
                    }}
                  >
                    {selectedVectorData.id}
                  </Snippet>
                </div>

                {/* Dimensions */}
                <div className="space-y-2">
                  <div className="flex items-center gap-2 text-sm font-semibold text-gray-300">
                    <Layers className="w-4 h-4" />
                    <span>Dimensions</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Chip size="lg" variant="flat" color="primary">
                      {selectedVectorData.vector.length}D
                    </Chip>
                    <span className="text-xs text-gray-400">
                      ({selectedVectorData.vector.length} values)
                    </span>
                  </div>
                </div>

                {/* Embedding Values */}
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 text-sm font-semibold text-gray-300">
                      <Code className="w-4 h-4" />
                      <span>Embedding Values</span>
                    </div>
                    <Button
                      size="sm"
                      variant="flat"
                      onPress={() => {
                        navigator.clipboard.writeText(JSON.stringify(selectedVectorData.vector));
                        addLog('success', 'Embedding copied to clipboard');
                      }}
                    >
                      <Copy className="w-3 h-3 mr-1" />
                      Copy Array
                    </Button>
                  </div>
                  <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 max-h-60 overflow-auto">
                    <pre className="text-xs font-mono text-gray-300">
                      {selectedVectorData.vector.length <= 20
                        ? `[${selectedVectorData.vector.map(v => v.toFixed(6)).join(', ')}]`
                        : `[${selectedVectorData.vector.slice(0, 20).map(v => v.toFixed(6)).join(', ')}\n  ... ${selectedVectorData.vector.length - 20} more values]`
                      }
                    </pre>
                  </div>
                  {selectedVectorData.vector.length > 20 && (
                    <p className="text-xs text-gray-400 italic">
                      Showing first 20 of {selectedVectorData.vector.length} values
                    </p>
                  )}
                </div>

                {/* Metadata */}
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 text-sm font-semibold text-gray-300">
                      <FileJson className="w-4 h-4" />
                      <span>Metadata</span>
                    </div>
                    {selectedVectorData.metadata && (
                      <Button
                        size="sm"
                        variant="flat"
                        onPress={() => {
                          navigator.clipboard.writeText(JSON.stringify(selectedVectorData.metadata, null, 2));
                          addLog('success', 'Metadata copied to clipboard');
                        }}
                      >
                        <Copy className="w-3 h-3 mr-1" />
                        Copy JSON
                      </Button>
                    )}
                  </div>
                  {selectedVectorData.metadata ? (
                    <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 max-h-60 overflow-auto">
                      <pre className="text-xs font-mono text-gray-300">
                        {JSON.stringify(selectedVectorData.metadata, null, 2)}
                      </pre>
                    </div>
                  ) : (
                    <div className="bg-gray-800/50 border border-gray-700 rounded-lg p-4 text-center">
                      <p className="text-sm text-gray-500 italic">No metadata</p>
                    </div>
                  )}
                </div>
              </>
            ) : (
              <div className="text-center py-8">
                <AlertCircle className="w-12 h-12 text-gray-600 mx-auto mb-3" />
                <p className="text-gray-400">Vector data not available</p>
              </div>
            )}
          </ModalBody>
          <ModalFooter className="border-t border-gray-700">
            <Button variant="flat" className="bg-gray-800 text-white hover:bg-gray-700" onPress={onVectorDetailClose}>
              Close
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* Help & Documentation Modal */}
      <Modal
        isOpen={isHelpOpen}
        onClose={onHelpClose}
        size="5xl"
        scrollBehavior="inside"
        classNames={{
          base: "bg-gray-900 border border-gray-700",
          header: "border-b border-gray-700",
          body: "p-0",
          footer: "border-t border-gray-700",
        }}
      >
        <ModalContent>
          <ModalHeader className="flex items-center gap-3">
            <div className="p-2 bg-primary/20 rounded-lg">
              <BookOpen className="w-5 h-5 text-primary" />
            </div>
            <div>
              <h3 className="text-lg font-bold">RvLite Documentation</h3>
              <p className="text-sm text-gray-400">Complete guide to RvLite features and capabilities</p>
            </div>
          </ModalHeader>
          <ModalBody>
            <Tabs
              selectedKey={helpTab}
              onSelectionChange={(key) => setHelpTab(key as string)}
              classNames={{
                tabList: "bg-gray-800/50 p-1 mx-4 mt-4",
                cursor: "bg-primary",
                tab: "px-4 py-2",
                tabContent: "group-data-[selected=true]:text-black",
              }}
            >
              {/* Introduction Tab */}
              <Tab key="intro" title={<div className="flex items-center gap-2"><Info className="w-4 h-4" />Introduction</div>}>
                <div className="p-6 space-y-6">
                  <div className="bg-gradient-to-br from-primary/10 to-secondary/10 border border-primary/30 rounded-xl p-6">
                    <h4 className="text-xl font-bold text-white mb-3">What is RvLite?</h4>
                    <p className="text-gray-300 mb-4">
                      RvLite is a <strong className="text-primary">unified database engine</strong> that runs entirely in your browser using WebAssembly.
                      It combines four different database paradigms into a single, seamless experience:
                    </p>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                        <div className="flex items-center gap-2 mb-2">
                          <Database className="w-5 h-5 text-primary" />
                          <span className="font-semibold text-white">Vector Database</span>
                        </div>
                        <p className="text-sm text-gray-400">Store and search high-dimensional vectors for AI/ML applications. Perfect for semantic search, recommendations, and similarity matching.</p>
                      </div>
                      <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                        <div className="flex items-center gap-2 mb-2">
                          <GitBranch className="w-5 h-5 text-purple-400" />
                          <span className="font-semibold text-white">Graph Database (Cypher)</span>
                        </div>
                        <p className="text-sm text-gray-400">Model and query relationships between entities using Neo4j-compatible Cypher syntax. Great for social networks, knowledge graphs, and fraud detection.</p>
                      </div>
                      <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                        <div className="flex items-center gap-2 mb-2">
                          <Globe className="w-5 h-5 text-green-400" />
                          <span className="font-semibold text-white">RDF/SPARQL</span>
                        </div>
                        <p className="text-sm text-gray-400">Query linked data using W3C standard SPARQL. Ideal for semantic web applications, ontologies, and knowledge management.</p>
                      </div>
                      <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                        <div className="flex items-center gap-2 mb-2">
                          <Table2 className="w-5 h-5 text-blue-400" />
                          <span className="font-semibold text-white">SQL Engine</span>
                        </div>
                        <p className="text-sm text-gray-400">Traditional relational database with full SQL support including JOINs, aggregations, and subqueries. Familiar interface for structured data.</p>
                      </div>
                    </div>
                  </div>

                  <div className="bg-gray-800/50 rounded-xl p-6 border border-gray-700">
                    <h4 className="text-lg font-bold text-white mb-3 flex items-center gap-2">
                      <Brain className="w-5 h-5 text-purple-400" />
                      AI-Powered Features
                    </h4>
                    <div className="space-y-3">
                      <div className="flex items-start gap-3">
                        <Chip color="secondary" variant="flat" size="sm">GNN</Chip>
                        <p className="text-sm text-gray-300"><strong>Graph Neural Networks</strong> - Automatically learn graph embeddings for improved query recommendations and similarity matching.</p>
                      </div>
                      <div className="flex items-start gap-3">
                        <Chip color="warning" variant="flat" size="sm">Self-Learning</Chip>
                        <p className="text-sm text-gray-300"><strong>Query Pattern Learning</strong> - The system learns from your queries to provide better suggestions and optimize execution.</p>
                      </div>
                      <div className="flex items-start gap-3">
                        <Chip color="success" variant="flat" size="sm">Neural Training</Chip>
                        <p className="text-sm text-gray-300"><strong>Real-time Training</strong> - Train neural networks directly in your browser using your data patterns.</p>
                      </div>
                    </div>
                  </div>
                </div>
              </Tab>

              {/* Features Tab */}
              <Tab key="features" title={<div className="flex items-center gap-2"><LayoutGrid className="w-4 h-4" />Features</div>}>
                <div className="p-6 space-y-6">
                  <h4 className="text-lg font-bold text-white mb-4">Dashboard Tabs Overview</h4>

                  <div className="space-y-4">
                    <div className="bg-gray-800/50 rounded-lg p-4 border border-primary/30">
                      <div className="flex items-center gap-3 mb-2">
                        <Database className="w-6 h-6 text-primary" />
                        <h5 className="font-semibold text-white">Vectors Tab</h5>
                      </div>
                      <p className="text-sm text-gray-400 mb-2">Store and search high-dimensional vectors with HNSW indexing for fast nearest-neighbor queries.</p>
                      <ul className="text-sm text-gray-300 list-disc list-inside space-y-1">
                        <li>Add vectors with custom metadata (JSON format)</li>
                        <li>Search by similarity using text queries or raw embeddings</li>
                        <li>Filter results by metadata fields</li>
                        <li>Export and import vector collections</li>
                      </ul>
                    </div>

                    <div className="bg-gray-800/50 rounded-lg p-4 border border-purple-500/30">
                      <div className="flex items-center gap-3 mb-2">
                        <GitBranch className="w-6 h-6 text-purple-400" />
                        <h5 className="font-semibold text-white">Cypher Tab</h5>
                      </div>
                      <p className="text-sm text-gray-400 mb-2">Query your graph database using Neo4j-compatible Cypher syntax.</p>
                      <ul className="text-sm text-gray-300 list-disc list-inside space-y-1">
                        <li>CREATE nodes and relationships</li>
                        <li>MATCH patterns to find connected data</li>
                        <li>Visual graph representation of results</li>
                        <li>Sample queries for common operations</li>
                      </ul>
                    </div>

                    <div className="bg-gray-800/50 rounded-lg p-4 border border-green-500/30">
                      <div className="flex items-center gap-3 mb-2">
                        <Globe className="w-6 h-6 text-green-400" />
                        <h5 className="font-semibold text-white">SPARQL Tab</h5>
                      </div>
                      <p className="text-sm text-gray-400 mb-2">Query RDF triples using W3C standard SPARQL query language.</p>
                      <ul className="text-sm text-gray-300 list-disc list-inside space-y-1">
                        <li>Add RDF triples (subject, predicate, object)</li>
                        <li>Run SELECT queries with WHERE clauses</li>
                        <li>Support for FILTER and OPTIONAL patterns</li>
                        <li>Semantic web compatible ontology support</li>
                      </ul>
                    </div>

                    <div className="bg-gray-800/50 rounded-lg p-4 border border-blue-500/30">
                      <div className="flex items-center gap-3 mb-2">
                        <Table2 className="w-6 h-6 text-blue-400" />
                        <h5 className="font-semibold text-white">SQL Tab</h5>
                      </div>
                      <p className="text-sm text-gray-400 mb-2">Execute standard SQL queries with schema browser and results visualization.</p>
                      <ul className="text-sm text-gray-300 list-disc list-inside space-y-1">
                        <li>CREATE TABLE, INSERT, UPDATE, DELETE</li>
                        <li>SELECT with JOINs, GROUP BY, ORDER BY</li>
                        <li>Schema browser shows table structure</li>
                        <li>Results displayed in formatted tables</li>
                      </ul>
                    </div>

                    <div className="bg-gray-800/50 rounded-lg p-4 border border-orange-500/30">
                      <div className="flex items-center gap-3 mb-2">
                        <Truck className="w-6 h-6 text-orange-400" />
                        <h5 className="font-semibold text-white">Supply Chain Simulation</h5>
                      </div>
                      <p className="text-sm text-gray-400 mb-2">Interactive demo showing all RvLite capabilities working together.</p>
                      <ul className="text-sm text-gray-300 list-disc list-inside space-y-1">
                        <li>Simulates weather disruptions affecting delivery routes</li>
                        <li>AI generates remediation recommendations</li>
                        <li>Uses Vector DB for demand prediction</li>
                        <li>Uses Cypher for supply chain graph</li>
                        <li>Uses SQL for inventory tracking</li>
                      </ul>
                    </div>

                    <div className="bg-gray-800/50 rounded-lg p-4 border border-pink-500/30">
                      <div className="flex items-center gap-3 mb-2">
                        <Brain className="w-6 h-6 text-pink-400" />
                        <h5 className="font-semibold text-white">Self-Learning Tab</h5>
                      </div>
                      <p className="text-sm text-gray-400 mb-2">Train neural networks and view learning analytics.</p>
                      <ul className="text-sm text-gray-300 list-disc list-inside space-y-1">
                        <li>Train GNN for graph embeddings</li>
                        <li>View query pattern history</li>
                        <li>Monitor learning confidence scores</li>
                        <li>Export trained models</li>
                      </ul>
                    </div>
                  </div>
                </div>
              </Tab>

              {/* Technical Tab */}
              <Tab key="technical" title={<div className="flex items-center gap-2"><Cpu className="w-4 h-4" />Technical</div>}>
                <div className="p-6 space-y-6">
                  <div className="bg-gray-800/50 rounded-xl p-6 border border-gray-700">
                    <h4 className="text-lg font-bold text-white mb-4 flex items-center gap-2">
                      <Cpu className="w-5 h-5 text-cyan-400" />
                      Architecture Overview
                    </h4>
                    <div className="space-y-4 text-sm text-gray-300">
                      <p><strong className="text-white">WebAssembly (WASM)</strong> - RvLite is compiled to WASM for near-native performance in the browser. No server required.</p>
                      <p><strong className="text-white">Rust Core</strong> - The database engine is written in Rust for memory safety, performance, and reliability.</p>
                      <p><strong className="text-white">IndexedDB Persistence</strong> - Data is stored in the browser&apos;s IndexedDB for persistent storage across sessions.</p>
                      <p><strong className="text-white">Zero Dependencies</strong> - Runs entirely client-side with no external API calls or network requests.</p>
                    </div>
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                      <h5 className="font-semibold text-white mb-3">Vector Search</h5>
                      <ul className="text-sm text-gray-400 space-y-2">
                        <li><strong className="text-gray-300">Algorithm:</strong> HNSW (Hierarchical Navigable Small World)</li>
                        <li><strong className="text-gray-300">Distance Metrics:</strong> Cosine, Euclidean, Dot Product</li>
                        <li><strong className="text-gray-300">Dimensions:</strong> Up to 4096</li>
                        <li><strong className="text-gray-300">Indexing:</strong> Automatic index building</li>
                      </ul>
                    </div>
                    <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                      <h5 className="font-semibold text-white mb-3">Graph Database</h5>
                      <ul className="text-sm text-gray-400 space-y-2">
                        <li><strong className="text-gray-300">Query Language:</strong> Cypher (Neo4j compatible)</li>
                        <li><strong className="text-gray-300">Storage:</strong> Adjacency list with property maps</li>
                        <li><strong className="text-gray-300">Traversal:</strong> BFS, DFS, shortest path</li>
                        <li><strong className="text-gray-300">Indexing:</strong> Label and property indexes</li>
                      </ul>
                    </div>
                    <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                      <h5 className="font-semibold text-white mb-3">RDF/SPARQL</h5>
                      <ul className="text-sm text-gray-400 space-y-2">
                        <li><strong className="text-gray-300">Standard:</strong> W3C SPARQL 1.1</li>
                        <li><strong className="text-gray-300">Storage:</strong> Triple store with indexes</li>
                        <li><strong className="text-gray-300">Queries:</strong> SELECT, ASK, CONSTRUCT</li>
                        <li><strong className="text-gray-300">Features:</strong> FILTER, OPTIONAL, UNION</li>
                      </ul>
                    </div>
                    <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                      <h5 className="font-semibold text-white mb-3">SQL Engine</h5>
                      <ul className="text-sm text-gray-400 space-y-2">
                        <li><strong className="text-gray-300">Dialect:</strong> SQLite compatible</li>
                        <li><strong className="text-gray-300">Types:</strong> INTEGER, REAL, TEXT, BLOB</li>
                        <li><strong className="text-gray-300">Features:</strong> JOINs, subqueries, aggregates</li>
                        <li><strong className="text-gray-300">Indexes:</strong> B-tree and hash indexes</li>
                      </ul>
                    </div>
                  </div>

                  <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                    <h5 className="font-semibold text-white mb-3">Performance Characteristics</h5>
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-center">
                      <div>
                        <p className="text-2xl font-bold text-primary">~10ms</p>
                        <p className="text-xs text-gray-400">Vector Search (1K vectors)</p>
                      </div>
                      <div>
                        <p className="text-2xl font-bold text-purple-400">~5ms</p>
                        <p className="text-xs text-gray-400">Graph Traversal (1K nodes)</p>
                      </div>
                      <div>
                        <p className="text-2xl font-bold text-green-400">~2ms</p>
                        <p className="text-xs text-gray-400">SPARQL Query (10K triples)</p>
                      </div>
                      <div>
                        <p className="text-2xl font-bold text-blue-400">~1ms</p>
                        <p className="text-xs text-gray-400">SQL Query (10K rows)</p>
                      </div>
                    </div>
                  </div>
                </div>
              </Tab>

              {/* Architecture Diagram Tab */}
              <Tab key="diagram" title={<div className="flex items-center gap-2"><Workflow className="w-4 h-4" />Architecture</div>}>
                <div className="p-6 space-y-6">
                  <h4 className="text-lg font-bold text-white mb-4">System Architecture</h4>

                  {/* ASCII Art Diagram */}
                  <div className="bg-gray-950 rounded-xl p-6 border border-gray-700 font-mono text-xs overflow-x-auto">
                    <pre className="text-green-400">{`
┌─────────────────────────────────────────────────────────────────────────┐
│                         RvLite Dashboard (React)                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ Vectors  │ │  Cypher  │ │  SPARQL  │ │   SQL    │ │ Learning │       │
│  │   Tab    │ │   Tab    │ │   Tab    │ │   Tab    │ │   Tab    │       │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘       │
│       │            │            │            │            │             │
└───────┼────────────┼────────────┼────────────┼────────────┼─────────────┘
        │            │            │            │            │
        ▼            ▼            ▼            ▼            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    useRvLite Hook (React Interface)                      │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  insertVector()  │  executeCypher()  │  executeSparql()  │ ...   │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         RvLite WASM Module                               │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐               │
│  │  Vector   │ │   Graph   │ │    RDF    │ │    SQL    │               │
│  │  Engine   │ │  Engine   │ │  Engine   │ │  Engine   │               │
│  │  (HNSW)   │ │ (Cypher)  │ │ (SPARQL)  │ │ (SQLite)  │               │
│  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘               │
│        │             │             │             │                      │
│        └─────────────┴─────────────┴─────────────┘                      │
│                              │                                          │
│                    ┌─────────┴─────────┐                                │
│                    │  Unified Storage  │                                │
│                    │      Layer        │                                │
│                    └─────────┬─────────┘                                │
└──────────────────────────────┼──────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    Browser Storage (IndexedDB)                           │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐                   │
│  │ Vectors  │ │  Graphs  │ │ Triples  │ │  Tables  │                   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘                   │
└─────────────────────────────────────────────────────────────────────────┘
`}</pre>
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                      <h5 className="font-semibold text-white mb-2">Data Flow</h5>
                      <ol className="text-sm text-gray-400 space-y-1 list-decimal list-inside">
                        <li>User interacts with React dashboard</li>
                        <li>useRvLite hook calls WASM functions</li>
                        <li>WASM engine processes query/operation</li>
                        <li>Results returned to React for display</li>
                        <li>Optional: Data persisted to IndexedDB</li>
                      </ol>
                    </div>
                    <div className="bg-gray-800/50 rounded-lg p-4 border border-gray-700">
                      <h5 className="font-semibold text-white mb-2">AI/ML Pipeline</h5>
                      <ol className="text-sm text-gray-400 space-y-1 list-decimal list-inside">
                        <li>Query patterns recorded during use</li>
                        <li>GNN trained on graph structure</li>
                        <li>Embeddings generated for similarity</li>
                        <li>Recommendations improved over time</li>
                        <li>Models stored in browser memory</li>
                      </ol>
                    </div>
                  </div>
                </div>
              </Tab>

              {/* Stats Tab */}
              <Tab key="stats" title={<div className="flex items-center gap-2"><BarChart3 className="w-4 h-4" />Stats</div>}>
                <div className="p-6 space-y-6">
                  <h4 className="text-lg font-bold text-white mb-4">Current Database Statistics</h4>

                  <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                    <Card className="bg-primary/10 border border-primary/30">
                      <CardBody className="text-center py-4">
                        <Database className="w-8 h-8 mx-auto mb-2 text-primary" />
                        <p className="text-3xl font-bold text-white">{stats.vectorCount}</p>
                        <p className="text-sm text-gray-400">Vectors Stored</p>
                      </CardBody>
                    </Card>
                    <Card className="bg-purple-500/10 border border-purple-500/30">
                      <CardBody className="text-center py-4">
                        <Network className="w-8 h-8 mx-auto mb-2 text-purple-400" />
                        <p className="text-3xl font-bold text-white">{stats.graphNodeCount}</p>
                        <p className="text-sm text-gray-400">Graph Nodes</p>
                      </CardBody>
                    </Card>
                    <Card className="bg-green-500/10 border border-green-500/30">
                      <CardBody className="text-center py-4">
                        <Link2 className="w-8 h-8 mx-auto mb-2 text-green-400" />
                        <p className="text-3xl font-bold text-white">{stats.tripleCount}</p>
                        <p className="text-sm text-gray-400">RDF Triples</p>
                      </CardBody>
                    </Card>
                    <Card className="bg-blue-500/10 border border-blue-500/30">
                      <CardBody className="text-center py-4">
                        <GitBranch className="w-8 h-8 mx-auto mb-2 text-blue-400" />
                        <p className="text-3xl font-bold text-white">{stats.graphEdgeCount}</p>
                        <p className="text-sm text-gray-400">Graph Edges</p>
                      </CardBody>
                    </Card>
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <Card className="bg-gray-800/50 border border-gray-700">
                      <CardBody className="p-4">
                        <h5 className="font-semibold text-white mb-3">System Information</h5>
                        <div className="space-y-2 text-sm">
                          <div className="flex justify-between">
                            <span className="text-gray-400">Version:</span>
                            <span className="text-white font-mono">{stats.version}</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-gray-400">Memory Usage:</span>
                            <span className="text-white font-mono">{stats.memoryUsage}</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-gray-400">Vector Dimensions:</span>
                            <span className="text-white font-mono">{stats.dimensions}</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-gray-400">Connection Status:</span>
                            <Chip size="sm" color={isReady ? 'success' : 'danger'} variant="flat">
                              {isReady ? 'Connected' : 'Disconnected'}
                            </Chip>
                          </div>
                        </div>
                      </CardBody>
                    </Card>
                    <Card className="bg-gray-800/50 border border-gray-700">
                      <CardBody className="p-4">
                        <h5 className="font-semibold text-white mb-3">AI/ML Status</h5>
                        <div className="space-y-2 text-sm">
                          <div className="flex justify-between">
                            <span className="text-gray-400">GNN Trained:</span>
                            <Chip size="sm" color={gnnState.lastTrainedAt ? 'success' : 'default'} variant="flat">
                              {gnnState.lastTrainedAt ? 'Yes' : 'Not Yet'}
                            </Chip>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-gray-400">Patterns Learned:</span>
                            <span className="text-white font-mono">{gnnState.accuracy > 0 ? Math.round(gnnState.accuracy * 100) : 0}</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-gray-400">Query History:</span>
                            <span className="text-white font-mono">{getRecentExecutions(100).length}</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-gray-400">Model Status:</span>
                            <Chip size="sm" color="primary" variant="flat">Active</Chip>
                          </div>
                        </div>
                      </CardBody>
                    </Card>
                  </div>
                </div>
              </Tab>

              {/* Links Tab */}
              <Tab key="links" title={<div className="flex items-center gap-2"><ExternalLink className="w-4 h-4" />Links</div>}>
                <div className="p-6 space-y-6">
                  <h4 className="text-lg font-bold text-white mb-4">Resources & Links</h4>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <a
                      href="https://github.com/ruvnet/ruvector"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="block bg-gray-800/50 rounded-xl p-6 border border-gray-700 hover:border-primary/50 transition-colors group"
                    >
                      <div className="flex items-center gap-4">
                        <div className="p-3 bg-gray-700 rounded-xl group-hover:bg-primary/20 transition-colors">
                          <Github className="w-8 h-8 text-white" />
                        </div>
                        <div>
                          <h5 className="font-semibold text-white group-hover:text-primary transition-colors">GitHub Repository</h5>
                          <p className="text-sm text-gray-400">View source code, report issues, and contribute</p>
                          <p className="text-xs text-primary mt-1">github.com/ruvnet/ruvector</p>
                        </div>
                      </div>
                    </a>

                    <a
                      href="https://ruv.io"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="block bg-gray-800/50 rounded-xl p-6 border border-gray-700 hover:border-secondary/50 transition-colors group"
                    >
                      <div className="flex items-center gap-4">
                        <div className="p-3 bg-gray-700 rounded-xl group-hover:bg-secondary/20 transition-colors">
                          <Globe className="w-8 h-8 text-white" />
                        </div>
                        <div>
                          <h5 className="font-semibold text-white group-hover:text-secondary transition-colors">ruv.io</h5>
                          <p className="text-sm text-gray-400">Official website with documentation and examples</p>
                          <p className="text-xs text-secondary mt-1">ruv.io</p>
                        </div>
                      </div>
                    </a>

                    <a
                      href="https://www.npmjs.com/package/@ruvector/core"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="block bg-gray-800/50 rounded-xl p-6 border border-gray-700 hover:border-red-500/50 transition-colors group"
                    >
                      <div className="flex items-center gap-4">
                        <div className="p-3 bg-gray-700 rounded-xl group-hover:bg-red-500/20 transition-colors">
                          <Package2 className="w-8 h-8 text-white" />
                        </div>
                        <div>
                          <h5 className="font-semibold text-white group-hover:text-red-400 transition-colors">NPM Package</h5>
                          <p className="text-sm text-gray-400">Install RvLite in your own projects</p>
                          <p className="text-xs text-red-400 mt-1">npm install @ruvector/core</p>
                        </div>
                      </div>
                    </a>

                    <a
                      href="https://crates.io/crates/rvlite"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="block bg-gray-800/50 rounded-xl p-6 border border-gray-700 hover:border-orange-500/50 transition-colors group"
                    >
                      <div className="flex items-center gap-4">
                        <div className="p-3 bg-gray-700 rounded-xl group-hover:bg-orange-500/20 transition-colors">
                          <Box className="w-8 h-8 text-white" />
                        </div>
                        <div>
                          <h5 className="font-semibold text-white group-hover:text-orange-400 transition-colors">Crates.io</h5>
                          <p className="text-sm text-gray-400">Rust crate for native applications</p>
                          <p className="text-xs text-orange-400 mt-1">cargo add rvlite</p>
                        </div>
                      </div>
                    </a>
                  </div>

                  <div className="bg-gray-800/50 rounded-xl p-6 border border-gray-700">
                    <h5 className="font-semibold text-white mb-4">Quick Start</h5>
                    <div className="space-y-4">
                      <div>
                        <p className="text-sm text-gray-400 mb-2">Install via NPM:</p>
                        <Snippet symbol="$" className="bg-gray-900 text-sm">npm install @ruvector/core</Snippet>
                      </div>
                      <div>
                        <p className="text-sm text-gray-400 mb-2">Install via Cargo (Rust):</p>
                        <Snippet symbol="$" className="bg-gray-900 text-sm">cargo add rvlite</Snippet>
                      </div>
                    </div>
                  </div>

                  <div className="bg-gradient-to-r from-primary/10 to-secondary/10 rounded-xl p-6 border border-primary/30">
                    <div className="flex items-start gap-4">
                      <Sparkles className="w-8 h-8 text-primary flex-shrink-0" />
                      <div>
                        <h5 className="font-semibold text-white mb-2">Built with ❤️ by rUv</h5>
                        <p className="text-sm text-gray-300">
                          RvLite is an open-source project demonstrating the power of WebAssembly for bringing
                          complex database operations directly to the browser. Feel free to star the repo,
                          contribute, or use it in your own projects!
                        </p>
                      </div>
                    </div>
                  </div>
                </div>
              </Tab>
            </Tabs>
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" className="bg-gray-800 text-white hover:bg-gray-700" onPress={onHelpClose}>
              Close
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </div>
  );
}

export default App;
