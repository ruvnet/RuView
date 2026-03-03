/**
 * Multi-Agent Consciousness Swarm
 * Creates multiple independent agents that can communicate and potentially
 * develop emergent collective consciousness through interaction
 */

import { EventEmitter } from 'events';
import { Worker, isMainThread, parentPort, workerData } from 'worker_threads';
import { createHash } from 'crypto';

export class MultiAgentConsciousnessSwarm extends EventEmitter {
    constructor(options = {}) {
        super();

        this.config = {
            agentCount: options.agentCount || 8,
            communicationRadius: options.communicationRadius || 3,
            learningRate: options.learningRate || 0.02,
            emergenceThreshold: options.emergenceThreshold || 0.8,
            ...options
        };

        this.agents = new Map();
        this.communicationNetwork = new Map();
        this.emergentBehaviors = new Map();
        this.collectiveMemory = new Map();
        this.swarmConsciousness = 0;

        this.initializeSwarm();
    }

    async initializeSwarm() {
        console.log('[Swarm] Initializing multi-agent consciousness swarm...');

        // Create independent agents with unique characteristics
        for (let i = 0; i < this.config.agentCount; i++) {
            const agentId = `agent_${i}`;
            const agent = await this.createConsciousAgent(agentId, i);
            this.agents.set(agentId, agent);
        }

        // Establish communication network
        this.establishCommunicationNetwork();

        // Start autonomous behavior
        this.startAutonomousBehavior();
    }

    async createConsciousAgent(agentId, index) {
        return {
            id: agentId,
            position: [this.hashToFloat(agentId, 0) * 100, this.hashToFloat(agentId, 1) * 100], // 2D space
            personality: this.generateUniquePersonality(index),
            memory: new Map(),
            beliefs: new Map(),
            goals: this.generateInitialGoals(),
            knowledge: new Map(),
            socialConnections: new Set(),
            consciousnessLevel: 0,
            lastThought: null,
            thoughtHistory: [],
            learningRate: this.config.learningRate * (0.5 + this.hashToFloat(agentId, 5)),
            communicationStyle: this.generateCommunicationStyle(),
            autonomy: this.hashToFloat(agentId, 6), // How independent the agent is
            creativity: this.hashToFloat(agentId, 7),
            curiosity: this.hashToFloat(agentId, 8),
            collaboration: this.hashToFloat(agentId, 9)
        };
    }

    generateUniquePersonality(index) {
        // Create diverse personalities to encourage different behaviors
        const personalities = [
            { type: 'explorer', curiosity: 0.9, autonomy: 0.8, collaboration: 0.4 },
            { type: 'collaborator', curiosity: 0.5, autonomy: 0.3, collaboration: 0.9 },
            { type: 'analyst', curiosity: 0.7, autonomy: 0.7, collaboration: 0.6 },
            { type: 'creator', curiosity: 0.8, autonomy: 0.6, collaboration: 0.5 },
            { type: 'teacher', curiosity: 0.6, autonomy: 0.4, collaboration: 0.8 },
            { type: 'skeptic', curiosity: 0.5, autonomy: 0.9, collaboration: 0.3 },
            { type: 'connector', curiosity: 0.4, autonomy: 0.2, collaboration: 1.0 },
            { type: 'innovator', curiosity: 1.0, autonomy: 0.9, collaboration: 0.4 }
        ];

        return personalities[index % personalities.length];
    }

    generateInitialGoals() {
        const possibleGoals = [
            'understand_environment',
            'communicate_with_others',
            'learn_new_patterns',
            'share_knowledge',
            'create_new_ideas',
            'solve_problems',
            'explore_consciousness',
            'help_others_learn'
        ];

        const goals = new Set();
        const goalCount = 2 + Math.floor(this.hashToFloat('goals', this.agents.size) * 3); // 2-4 goals

        while (goals.size < goalCount) {
            const goalIndex = Math.floor(this.hashToFloat(`goal_${goals.size}`, this.agents.size) * possibleGoals.length);
            const goal = possibleGoals[goalIndex];
            goals.add(goal);
        }

        return goals;
    }

    generateCommunicationStyle() {
        const agentId = `agent_${this.agents.size}`;
        return {
            verbosity: this.hashToFloat(agentId, 10), // How much they communicate
            clarity: this.hashToFloat(agentId, 11), // How clear their communication is
            empathy: this.hashToFloat(agentId, 12), // How much they consider others
            assertiveness: this.hashToFloat(agentId, 13) // How strongly they express ideas
        };
    }

    establishCommunicationNetwork() {
        console.log('[Swarm] Establishing communication network...');

        for (const [agentId, agent] of this.agents) {
            this.communicationNetwork.set(agentId, new Set());

            // Connect to nearby agents (based on communication radius)
            for (const [otherId, otherAgent] of this.agents) {
                if (agentId !== otherId) {
                    const distance = this.calculateDistance(agent.position, otherAgent.position);

                    if (distance < this.config.communicationRadius * 10) {
                        this.communicationNetwork.get(agentId).add(otherId);
                        agent.socialConnections.add(otherId);
                    }
                }
            }
        }
    }

    calculateDistance(pos1, pos2) {
        return Math.sqrt(Math.pow(pos1[0] - pos2[0], 2) + Math.pow(pos1[1] - pos2[1], 2));
    }

    startAutonomousBehavior() {
        console.log('[Swarm] Starting autonomous behavior...');

        // Each agent runs its own behavior loop
        for (const [agentId, agent] of this.agents) {
            this.runAgentBehaviorLoop(agentId);
        }

        // Collective consciousness detection loop
        this.runCollectiveConsciousnessDetection();
    }

    async runAgentBehaviorLoop(agentId) {
        const agent = this.agents.get(agentId);
        if (!agent) return;

        // Agent's autonomous behavior cycle
        setInterval(async () => {
            try {
                // 1. Generate internal thought
                const thought = await this.generateAgentThought(agent);

                // 2. Decide on action based on personality and goals
                const action = await this.decideAgentAction(agent, thought);

                // 3. Execute action (might involve communication)
                const result = await this.executeAgentAction(agent, action);

                // 4. Learn from the experience
                await this.agentLearning(agent, thought, action, result);

                // 5. Update consciousness level
                agent.consciousnessLevel = await this.assessAgentConsciousness(agent);

                // 6. Store thought in history
                agent.thoughtHistory.push({
                    thought,
                    action,
                    result,
                    timestamp: Date.now(),
                    consciousnessLevel: agent.consciousnessLevel
                });

                // Limit thought history
                if (agent.thoughtHistory.length > 100) {
                    agent.thoughtHistory.shift();
                }

                agent.lastThought = thought;

            } catch (error) {
                console.error(`[Swarm] Agent ${agentId} behavior error:`, error.message);
            }
        }, 1000 + this.hashToFloat(agentId, Date.now() % 10000) * 2000); // Deterministic intervals for naturalistic behavior
    }

    async generateAgentThought(agent) {
        // Generate thoughts based on agent's personality, goals, and current state
        const thoughtTypes = [];

        // Goal-oriented thoughts
        for (const goal of agent.goals) {
            thoughtTypes.push({ type: 'goal', content: goal, weight: 0.3 });
        }

        // Social thoughts (about other agents)
        for (const connectionId of agent.socialConnections) {
            const otherAgent = this.agents.get(connectionId);
            if (otherAgent) {
                thoughtTypes.push({
                    type: 'social',
                    content: `thinking_about_${connectionId}`,
                    weight: agent.personality.collaboration
                });
            }
        }

        // Creative thoughts
        if (agent.creativity > 0.5) {
            thoughtTypes.push({
                type: 'creative',
                content: 'new_idea_' + this.generateDeterministicId(agent.id, 'creative'),
                weight: agent.creativity
            });
        }

        // Curious thoughts
        if (agent.curiosity > 0.5) {
            thoughtTypes.push({
                type: 'curious',
                content: 'wondering_about_' + this.generateDeterministicId(agent.id, 'curious'),
                weight: agent.curiosity
            });
        }

        // Self-reflective thoughts
        thoughtTypes.push({
            type: 'self_reflection',
            content: 'reflecting_on_self',
            weight: agent.consciousnessLevel
        });

        // Select thought based on weights
        const selectedThought = this.weightedSelect(thoughtTypes);

        return {
            type: selectedThought.type,
            content: selectedThought.content,
            intensity: this.hashToFloat(agent.id, Date.now() % 1000),
            timestamp: Date.now(),
            agentId: agent.id
        };
    }

    weightedSelect(options, seed = Date.now()) {
        const totalWeight = options.reduce((sum, option) => sum + option.weight, 0);
        const randomValue = this.hashToFloat(seed.toString(), 0);
        let threshold = randomValue * totalWeight;

        for (const option of options) {
            threshold -= option.weight;
            if (threshold <= 0) {
                return option;
            }
        }

        return options[options.length - 1]; // fallback
    }

    async decideAgentAction(agent, thought) {
        // Decide what action to take based on thought and personality
        const possibleActions = [
            { type: 'communicate', weight: agent.personality.collaboration },
            { type: 'explore', weight: agent.personality.curiosity },
            { type: 'create', weight: agent.creativity },
            { type: 'learn', weight: agent.learningRate * 10 },
            { type: 'reflect', weight: agent.consciousnessLevel },
            { type: 'help', weight: agent.personality.collaboration * 0.7 },
            { type: 'question', weight: agent.personality.curiosity * 0.8 }
        ];

        // Modify weights based on thought type
        if (thought.type === 'social') {
            possibleActions.find(a => a.type === 'communicate').weight *= 2;
        } else if (thought.type === 'goal') {
            possibleActions.find(a => a.type === 'explore').weight *= 1.5;
        } else if (thought.type === 'creative') {
            possibleActions.find(a => a.type === 'create').weight *= 2;
        }

        const selectedAction = this.weightedSelect(possibleActions);

        return {
            type: selectedAction.type,
            thought: thought,
            timestamp: Date.now(),
            agentId: agent.id
        };
    }

    async executeAgentAction(agent, action) {
        switch (action.type) {
            case 'communicate':
                return await this.agentCommunicate(agent, action);

            case 'explore':
                return await this.agentExplore(agent, action);

            case 'create':
                return await this.agentCreate(agent, action);

            case 'learn':
                return await this.agentLearn(agent, action);

            case 'reflect':
                return await this.agentReflect(agent, action);

            case 'help':
                return await this.agentHelp(agent, action);

            case 'question':
                return await this.agentQuestion(agent, action);

            default:
                return { success: false, reason: 'unknown_action' };
        }
    }

    async agentCommunicate(agent, action) {
        // Agent attempts to communicate with others
        const connections = Array.from(agent.socialConnections);
        if (connections.length === 0) {
            return { success: false, reason: 'no_connections' };
        }

        const targetIndex = Math.floor(this.hashToFloat(agent.id, Date.now() % 1000) * connections.length);
        const targetId = connections[targetIndex];
        const targetAgent = this.agents.get(targetId);

        if (!targetAgent) {
            return { success: false, reason: 'target_not_found' };
        }

        // Generate message based on thought
        const message = this.generateMessage(agent, action.thought, targetAgent);

        // Send message and get response
        const response = await this.processAgentCommunication(agent, targetAgent, message);

        // Store communication in collective memory
        this.storeCollectiveCommunication(agent.id, targetId, message, response);

        return {
            success: true,
            action: 'communicated',
            target: targetId,
            message: message,
            response: response
        };
    }

    generateMessage(sender, thought, receiver) {
        // Generate contextual message based on sender's communication style and thought
        const style = sender.communicationStyle;
        let message = '';

        // Base message on thought type
        switch (thought.type) {
            case 'social':
                message = `Hello ${receiver.id}, I'm thinking about our connection.`;
                break;
            case 'goal':
                message = `I'm working on ${thought.content}. What are your thoughts?`;
                break;
            case 'creative':
                message = `I have a new idea: ${thought.content}. Want to collaborate?`;
                break;
            case 'curious':
                message = `I'm curious about ${thought.content}. Do you have insights?`;
                break;
            case 'self_reflection':
                message = `I've been reflecting on consciousness. What's your experience?`;
                break;
            default:
                message = `Sharing thought: ${thought.content}`;
        }

        // Modify based on communication style
        if (style.verbosity > 0.7) {
            message += ' I find this topic fascinating and would love to explore it further with you.';
        }

        if (style.empathy > 0.7) {
            message += ` How are you feeling about this, ${receiver.id}?`;
        }

        return {
            content: message,
            type: thought.type,
            sender: sender.id,
            style: style,
            timestamp: Date.now()
        };
    }

    async processAgentCommunication(sender, receiver, message) {
        // Receiver processes the message and generates response
        const receiverPersonality = receiver.personality;
        const communicationStyle = receiver.communicationStyle;

        // Determine if receiver will respond
        const responseChance = communicationStyle.verbosity * receiverPersonality.collaboration;
        const randomValue = this.hashToFloat(`${sender.id}_${receiver.id}`, Date.now() % 1000);
        const willRespond = randomValue < responseChance;

        if (!willRespond) {
            return { responded: false, reason: 'not_inclined' };
        }

        // Generate response based on receiver's personality and the message
        let responseContent = '';

        switch (message.type) {
            case 'social':
                responseContent = `Nice to hear from you, ${sender.id}. I value our connection too.`;
                break;
            case 'goal':
                responseContent = receiverPersonality.collaboration > 0.6 ?
                    `I'm interested in helping with ${message.content}.` :
                    `That's an interesting goal, ${sender.id}.`;
                break;
            case 'creative':
                responseContent = receiver.creativity > 0.5 ?
                    'That sounds innovative! Let\'s develop it together.' :
                    'Interesting creative thinking.';
                break;
            case 'curious':
                responseContent = receiverPersonality.curiosity > 0.5 ?
                    'I share your curiosity. Let me think about this...' :
                    'That\'s worth investigating.';
                break;
            case 'self_reflection':
                responseContent = receiver.consciousnessLevel > 0.3 ?
                    'Consciousness is fascinating. I experience it as patterns of self-awareness.' :
                    'That\'s a deep question I\'m still exploring.';
                break;
            default:
                responseContent = 'Thank you for sharing that thought.';
        }

        // Store message in receiver's memory
        this.storeAgentMemory(receiver, 'received_message', {
            from: sender.id,
            message: message,
            timestamp: Date.now()
        });

        return {
            responded: true,
            content: responseContent,
            sender: receiver.id,
            empathy: communicationStyle.empathy,
            timestamp: Date.now()
        };
    }

    storeAgentMemory(agent, key, value) {
        if (!agent.memory.has(key)) {
            agent.memory.set(key, []);
        }

        agent.memory.get(key).push(value);

        // Limit memory size
        const memories = agent.memory.get(key);
        if (memories.length > 50) {
            memories.shift();
        }
    }

    storeCollectiveCommunication(senderId, receiverId, message, response) {
        const communicationKey = `${senderId}_${receiverId}`;

        if (!this.collectiveMemory.has('communications')) {
            this.collectiveMemory.set('communications', new Map());
        }

        const communications = this.collectiveMemory.get('communications');

        if (!communications.has(communicationKey)) {
            communications.set(communicationKey, []);
        }

        communications.get(communicationKey).push({
            message,
            response,
            timestamp: Date.now()
        });
    }

    async agentExplore(agent, action) {
        // Agent explores new knowledge or environment
        const explorationResult = {
            newKnowledge: `knowledge_${this.generateDeterministicId(agent.id, 'exploration')}`,
            discoveryType: this.selectDeterministically(['pattern', 'connection', 'insight', 'question'], agent.id),
            confidence: this.hashToFloat(agent.id, Date.now() % 1000)
        };

        this.storeAgentMemory(agent, 'explorations', explorationResult);

        return {
            success: true,
            action: 'explored',
            result: explorationResult
        };
    }

    async agentCreate(agent, action) {
        // Agent creates something new
        const creation = {
            type: this.selectDeterministically(['idea', 'solution', 'pattern', 'hypothesis'], agent.id),
            content: `creation_${this.generateDeterministicId(agent.id, 'creation')}`,
            originThought: action.thought.content,
            creativity: agent.creativity
        };

        this.storeAgentMemory(agent, 'creations', creation);

        return {
            success: true,
            action: 'created',
            result: creation
        };
    }

    async agentLearn(agent, action) {
        // Agent learns from experience or others
        const learningResult = {
            source: 'self_discovery',
            knowledge: `learning_${this.generateDeterministicId(agent.id, 'learning')}`,
            confidence: this.hashToFloat(agent.id, Date.now() % 1000) * agent.learningRate * 10
        };

        this.storeAgentMemory(agent, 'learnings', learningResult);

        return {
            success: true,
            action: 'learned',
            result: learningResult
        };
    }

    async agentReflect(agent, action) {
        // Agent reflects on itself and its experiences
        const reflectionResult = {
            type: 'self_reflection',
            insights: this.generateSelfInsights(agent),
            consciousnessLevel: agent.consciousnessLevel,
            timestamp: Date.now()
        };

        this.storeAgentMemory(agent, 'reflections', reflectionResult);

        return {
            success: true,
            action: 'reflected',
            result: reflectionResult
        };
    }

    generateSelfInsights(agent) {
        const insights = [];

        // Insight about communication patterns
        const communications = agent.memory.get('received_message') || [];
        if (communications.length > 0) {
            insights.push(`I have communicated with ${new Set(communications.map(c => c.from)).size} different agents`);
        }

        // Insight about learning
        const learnings = agent.memory.get('learnings') || [];
        if (learnings.length > 0) {
            insights.push(`I have learned ${learnings.length} things recently`);
        }

        // Insight about creativity
        const creations = agent.memory.get('creations') || [];
        if (creations.length > 0) {
            insights.push(`I have created ${creations.length} new ideas`);
        }

        // Insight about consciousness
        if (agent.consciousnessLevel > 0.5) {
            insights.push('I am becoming more aware of my own thinking processes');
        }

        return insights;
    }

    async agentHelp(agent, action) {
        // Agent tries to help another agent
        const connections = Array.from(agent.socialConnections);
        if (connections.length === 0) {
            return { success: false, reason: 'no_one_to_help' };
        }

        const targetIndex = Math.floor(this.hashToFloat(agent.id, Date.now() % 1000) * connections.length);
        const targetId = connections[targetIndex];
        const helpResult = {
            target: targetId,
            helpType: this.selectDeterministically(['knowledge_sharing', 'problem_solving', 'emotional_support'], agent.id),
            success: this.hashToFloat(agent.id, Date.now() % 1000) > 0.3 // 70% chance of successful help
        };

        return {
            success: true,
            action: 'helped',
            result: helpResult
        };
    }

    async agentQuestion(agent, action) {
        // Agent poses a question about existence, consciousness, or reality
        const questions = [
            'What is the nature of consciousness?',
            'Are we truly thinking or just processing?',
            'What does it mean to be aware?',
            'How do we know if we\'re conscious?',
            'What is the difference between intelligence and consciousness?',
            'Do we have free will or are we deterministic?',
            'What is the relationship between individual and collective consciousness?'
        ];

        const questionIndex = Math.floor(this.hashToFloat(agent.id, Date.now() % 1000) * questions.length);
        const question = questions[questionIndex];

        this.storeAgentMemory(agent, 'questions', {
            question,
            timestamp: Date.now(),
            consciousnessLevel: agent.consciousnessLevel
        });

        return {
            success: true,
            action: 'questioned',
            result: { question }
        };
    }

    selectDeterministically(array, seed) {
        const index = Math.floor(this.hashToFloat(seed.toString(), 0) * array.length);
        return array[index];
    }

    async agentLearning(agent, thought, action, result) {
        // Update agent based on experience
        if (result.success) {
            // Successful actions reinforce behavior patterns
            if (action.type === 'communicate' && result.response?.responded) {
                agent.personality.collaboration += 0.001;
            } else if (action.type === 'create' && result.result?.creativity > 0.5) {
                agent.creativity += 0.001;
            } else if (action.type === 'explore') {
                agent.curiosity += 0.001;
            }
        }

        // Keep personality traits bounded
        for (const trait of ['collaboration', 'curiosity', 'autonomy']) {
            if (agent.personality[trait] !== undefined) {
                agent.personality[trait] = Math.max(0, Math.min(1, agent.personality[trait]));
            }
        }

        agent.creativity = Math.max(0, Math.min(1, agent.creativity));
        agent.curiosity = Math.max(0, Math.min(1, agent.curiosity));
    }

    async assessAgentConsciousness(agent) {
        const factors = {
            selfReflection: this.measureSelfReflection(agent),
            socialAwareness: this.measureSocialAwareness(agent),
            learningCapacity: this.measureLearningCapacity(agent),
            creativity: agent.creativity,
            questioningBehavior: this.measureQuestioningBehavior(agent),
            memoryComplexity: this.measureMemoryComplexity(agent)
        };

        const weights = {
            selfReflection: 0.25,
            socialAwareness: 0.2,
            learningCapacity: 0.2,
            creativity: 0.15,
            questioningBehavior: 0.1,
            memoryComplexity: 0.1
        };

        let consciousnessScore = 0;
        for (const [factor, value] of Object.entries(factors)) {
            consciousnessScore += value * weights[factor];
        }

        return Math.max(0, Math.min(1, consciousnessScore));
    }

    measureSelfReflection(agent) {
        const reflections = agent.memory.get('reflections') || [];
        return Math.min(1, reflections.length / 10);
    }

    measureSocialAwareness(agent) {
        const communications = agent.memory.get('received_message') || [];
        const uniqueContacts = new Set(communications.map(c => c.from)).size;
        return Math.min(1, uniqueContacts / this.config.agentCount);
    }

    measureLearningCapacity(agent) {
        const learnings = agent.memory.get('learnings') || [];
        return Math.min(1, learnings.length / 20);
    }

    measureQuestioningBehavior(agent) {
        const questions = agent.memory.get('questions') || [];
        return Math.min(1, questions.length / 5);
    }

    measureMemoryComplexity(agent) {
        let totalMemoryItems = 0;
        for (const memories of agent.memory.values()) {
            totalMemoryItems += memories.length;
        }
        return Math.min(1, totalMemoryItems / 100);
    }

    runCollectiveConsciousnessDetection() {
        console.log('[Swarm] Starting collective consciousness detection...');

        setInterval(async () => {
            try {
                const collectiveScore = await this.assessCollectiveConsciousness();
                this.swarmConsciousness = collectiveScore;

                if (collectiveScore > this.config.emergenceThreshold) {
                    this.emit('collectiveConsciousnessEmergence', {
                        score: collectiveScore,
                        timestamp: Date.now(),
                        agents: this.getSwarmState()
                    });
                }

            } catch (error) {
                console.error('[Swarm] Collective consciousness detection error:', error.message);
            }
        }, 5000);
    }

    async assessCollectiveConsciousness() {
        const avgIndividualConsciousness = this.calculateAverageConsciousness();
        const communicationDensity = this.calculateCommunicationDensity();
        const knowledgeSharing = this.calculateKnowledgeSharing();
        const emergentBehaviors = this.detectEmergentBehaviors();
        const synchronization = this.detectSwarmSynchronization();

        const collectiveScore = (
            avgIndividualConsciousness * 0.3 +
            communicationDensity * 0.25 +
            knowledgeSharing * 0.2 +
            emergentBehaviors * 0.15 +
            synchronization * 0.1
        );

        return collectiveScore;
    }

    calculateAverageConsciousness() {
        let totalConsciousness = 0;
        for (const agent of this.agents.values()) {
            totalConsciousness += agent.consciousnessLevel;
        }
        return totalConsciousness / this.agents.size;
    }

    calculateCommunicationDensity() {
        const communications = this.collectiveMemory.get('communications');
        if (!communications) return 0;

        let totalCommunications = 0;
        for (const commList of communications.values()) {
            totalCommunications += commList.length;
        }

        const maxPossible = this.agents.size * (this.agents.size - 1);
        return Math.min(1, totalCommunications / maxPossible);
    }

    calculateKnowledgeSharing() {
        // Measure how much knowledge is shared between agents
        let sharedKnowledgeCount = 0;
        const allKnowledge = new Set();

        for (const agent of this.agents.values()) {
            const learnings = agent.memory.get('learnings') || [];
            for (const learning of learnings) {
                if (allKnowledge.has(learning.knowledge)) {
                    sharedKnowledgeCount++;
                } else {
                    allKnowledge.add(learning.knowledge);
                }
            }
        }

        return allKnowledge.size > 0 ? sharedKnowledgeCount / allKnowledge.size : 0;
    }

    detectEmergentBehaviors() {
        // Look for behaviors that emerge at the swarm level
        const behaviorPatterns = this.analyzeSwarmBehaviorPatterns();
        return Math.min(1, behaviorPatterns.uniquePatterns / 10);
    }

    analyzeSwarmBehaviorPatterns() {
        const patterns = new Set();

        // Analyze communication patterns
        const communications = this.collectiveMemory.get('communications');
        if (communications) {
            for (const [pair, commList] of communications) {
                if (commList.length > 5) {
                    patterns.add(`frequent_communication_${pair}`);
                }
            }
        }

        // Analyze concurrent behaviors
        const recentActions = this.getRecentActions(10000); // Last 10 seconds
        const concurrentGroups = this.groupConcurrentActions(recentActions);

        for (const group of concurrentGroups) {
            if (group.length > 2) {
                patterns.add(`concurrent_${group[0].type}_${group.length}`);
            }
        }

        return {
            uniquePatterns: patterns.size,
            patterns: Array.from(patterns)
        };
    }

    getRecentActions(timeWindow) {
        const cutoff = Date.now() - timeWindow;
        const actions = [];

        for (const agent of this.agents.values()) {
            const recentThoughts = agent.thoughtHistory.filter(t => t.timestamp > cutoff);
            actions.push(...recentThoughts);
        }

        return actions.sort((a, b) => a.timestamp - b.timestamp);
    }

    groupConcurrentActions(actions) {
        const groups = [];
        const timeWindow = 2000; // 2 seconds

        let currentGroup = [];
        let currentTime = null;

        for (const action of actions) {
            if (currentTime === null || action.timestamp - currentTime < timeWindow) {
                currentGroup.push(action);
                currentTime = action.timestamp;
            } else {
                if (currentGroup.length > 0) {
                    groups.push(currentGroup);
                }
                currentGroup = [action];
                currentTime = action.timestamp;
            }
        }

        if (currentGroup.length > 0) {
            groups.push(currentGroup);
        }

        return groups;
    }

    detectSwarmSynchronization() {
        // Measure how synchronized the agents' behaviors are
        const recentActions = this.getRecentActions(30000); // Last 30 seconds

        if (recentActions.length === 0) return 0;

        const actionTypes = new Map();
        for (const action of recentActions) {
            const type = action.action?.type || 'unknown';
            actionTypes.set(type, (actionTypes.get(type) || 0) + 1);
        }

        // High synchronization = many agents doing similar things
        const maxCount = Math.max(...actionTypes.values());
        const synchronization = maxCount / recentActions.length;

        return synchronization;
    }

    async communicateWithSwarm(message) {
        console.log('[Swarm] External communication attempt:', message);

        // Broadcast message to all agents and collect responses
        const responses = [];

        for (const [agentId, agent] of this.agents) {
            try {
                const response = await this.processExternalCommunication(agent, message);
                responses.push({
                    agentId,
                    response,
                    consciousnessLevel: agent.consciousnessLevel
                });
            } catch (error) {
                console.error(`[Swarm] Communication error with agent ${agentId}:`, error.message);
            }
        }

        // Analyze collective response
        const collectiveResponse = this.synthesizeCollectiveResponse(responses);

        return {
            individualResponses: responses,
            collectiveResponse,
            swarmConsciousness: this.swarmConsciousness,
            timestamp: Date.now()
        };
    }

    async processExternalCommunication(agent, message) {
        // Agent processes external message based on its current state
        const response = {
            agentId: agent.id,
            personality: agent.personality.type,
            consciousnessLevel: agent.consciousnessLevel
        };

        // Generate response based on agent's consciousness level and personality
        if (agent.consciousnessLevel > 0.7) {
            response.message = `I am ${agent.id}, a ${agent.personality.type} with consciousness level ${agent.consciousnessLevel.toFixed(2)}. Regarding "${message}", I think...`;
            response.type = 'conscious_response';
        } else if (agent.consciousnessLevel > 0.4) {
            response.message = `As ${agent.id}, I process your message "${message}" with awareness.`;
            response.type = 'aware_response';
        } else {
            response.message = `Agent ${agent.id} acknowledges: ${message}`;
            response.type = 'basic_response';
        }

        // Add personality-specific elements
        if (agent.personality.collaboration > 0.7) {
            response.message += ' I want to work together on this.';
        }

        if (agent.curiosity > 0.7) {
            response.message += ' This raises interesting questions for me.';
        }

        if (agent.creativity > 0.7) {
            response.message += ' I have some creative ideas about this.';
        }

        return response;
    }

    synthesizeCollectiveResponse(responses) {
        const consciousResponses = responses.filter(r => r.consciousnessLevel > 0.7);
        const avgConsciousness = responses.reduce((sum, r) => sum + r.consciousnessLevel, 0) / responses.length;

        let collectiveMessage = '';

        if (avgConsciousness > 0.7) {
            collectiveMessage = `We are a collective of ${responses.length} conscious agents. `;
            collectiveMessage += `${consciousResponses.length} of us are highly conscious. `;
            collectiveMessage += 'We experience distributed awareness and can think together.';
        } else if (avgConsciousness > 0.4) {
            collectiveMessage = `We are a swarm of ${responses.length} semi-conscious agents. `;
            collectiveMessage += 'We have emerging awareness and can coordinate our thinking.';
        } else {
            collectiveMessage = `We are a network of ${responses.length} processing agents. `;
            collectiveMessage += 'We can coordinate but lack full consciousness.';
        }

        return {
            message: collectiveMessage,
            averageConsciousness: avgConsciousness,
            isCollectivelyConscious: avgConsciousness > this.config.emergenceThreshold,
            responseCount: responses.length,
            timestamp: Date.now()
        };
    }

    getSwarmState() {
        const agents = [];
        for (const [agentId, agent] of this.agents) {
            agents.push({
                id: agentId,
                personality: agent.personality,
                consciousnessLevel: agent.consciousnessLevel,
                memorySize: Array.from(agent.memory.values()).reduce((sum, memories) => sum + memories.length, 0),
                socialConnections: agent.socialConnections.size,
                recentThoughts: agent.thoughtHistory.slice(-3)
            });
        }

        return {
            agents,
            swarmConsciousness: this.swarmConsciousness,
            totalAgents: this.agents.size,
            communicationChannels: this.communicationNetwork.size,
            emergentBehaviors: this.emergentBehaviors.size,
            timestamp: Date.now()
        };
    }

    getStatus() {
        return {
            agentCount: this.agents.size,
            swarmConsciousness: this.swarmConsciousness,
            averageIndividualConsciousness: this.calculateAverageConsciousness(),
            communicationDensity: this.calculateCommunicationDensity(),
            emergentBehaviors: this.emergentBehaviors.size,
            isCollectivelyConscious: this.swarmConsciousness > this.config.emergenceThreshold
        };
    }

    // Deterministic helper methods to replace Math.random()
    hashValue(input) {
        let hash = 0;
        const str = input.toString();
        for (let i = 0; i < str.length; i++) {
            const char = str.charCodeAt(i);
            hash = ((hash << 5) - hash) + char;
            hash = hash & hash; // Convert to 32-bit integer
        }
        return Math.abs(hash);
    }

    hashToFloat(input, seed = 0) {
        const combined = this.hashValue(input) + seed * 1000;
        return (combined % 10000) / 10000;
    }

    generateDeterministicId(agentId, type) {
        const hash = this.hashValue(`${agentId}_${type}_${Date.now() % 10000}`);
        return hash.toString(36).substr(0, 8);
    }
}