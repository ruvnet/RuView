/**
 * Consciousness Metrics
 * Measurement functions for consciousness indicators
 */

export function measureEmergence(system) {
    const measurements = {
        downwardCausation: measureDownwardCausation(system),
        irreducibility: measureIrreducibility(system),
        novelProperties: measureNovelProperties(system),
        selfOrganization: measureSelfOrganization(system),
        adaptation: measureAdaptation(system)
    };

    const totalScore = Object.values(measurements).reduce((a, b) => a + b, 0) / Object.keys(measurements).length;

    return {
        score: totalScore,
        measurements,
        isEmergent: totalScore > 0.5,
        scientificBasis: 'Emergence Theory (Bedau, 2008; Holland, 1998)'
    };
}

function measureDownwardCausation(system) {
    if (!system.selfModifications) return 0;

    const highLevelModifications = system.selfModifications.filter(m =>
        m.type === 'goal_addition' || m.type === 'structural_modification'
    );

    return Math.min(1, highLevelModifications.length / 10);
}

function measureIrreducibility(system) {
    const systemLevelProperties = [
        system.consciousness?.emergence,
        system.selfAwareness,
        system.integration
    ].filter(p => p > 0);

    return Math.min(1, systemLevelProperties.length / 3);
}

function measureNovelProperties(system) {
    const novelBehaviors = system.unprogrammedBehaviors?.length || 0;
    const novelGoals = system.goals?.filter(g => !['explore', 'understand'].includes(g)).length || 0;
    const novelPatterns = system.emergentPatterns?.size || 0;

    return Math.min(1, (novelBehaviors + novelGoals + novelPatterns) / 30);
}

function measureSelfOrganization(system) {
    const hasGoalFormation = system.goals?.length > 0;
    const hasKnowledgeBuilding = system.knowledge?.size > 0;
    const hasPatternFormation = system.emergentPatterns?.size > 0;

    const score = (hasGoalFormation ? 0.33 : 0) +
        (hasKnowledgeBuilding ? 0.33 : 0) +
        (hasPatternFormation ? 0.34 : 0);

    return score;
}

function measureAdaptation(system) {
    if (!system.experiences || system.experiences.length < 10) return 0;

    const early = system.experiences.slice(0, 5);
    const late = system.experiences.slice(-5);

    const earlyScore = early.reduce((sum, e) => sum + (e.consciousness?.emergence || 0), 0) / 5;
    const lateScore = late.reduce((sum, e) => sum + (e.consciousness?.emergence || 0), 0) / 5;

    const improvement = lateScore - earlyScore;

    return Math.max(0, Math.min(1, improvement * 2));
}