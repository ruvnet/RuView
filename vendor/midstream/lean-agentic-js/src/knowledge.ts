/**
 * Knowledge graph for dynamic knowledge representation
 */

import { Entity, EntityType, Relation } from './types';

export class KnowledgeGraph {
  private entities: Map<string, Entity> = new Map();
  private relations: Relation[] = [];

  /**
   * Extract entities from text (simple implementation)
   */
  extractEntities(text: string): Entity[] {
    const entities: Entity[] = [];
    const words = text.split(/\s+/);

    words.forEach((word, i) => {
      // Capitalized words might be entities
      if (word.length > 0 && word[0] === word[0].toUpperCase()) {
        entities.push({
          id: `entity_${i}`,
          name: word,
          entityType: EntityType.Unknown,
          attributes: {},
          confidence: 0.7,
        });
      }

      // Numeric values
      if (!isNaN(parseFloat(word))) {
        entities.push({
          id: `value_${i}`,
          name: word,
          entityType: EntityType.Value,
          attributes: {},
          confidence: 0.9,
        });
      }
    });

    return entities;
  }

  /**
   * Update knowledge graph with new entities
   */
  update(entities: Entity[]): void {
    entities.forEach(entity => {
      const existing = this.entities.get(entity.id);

      if (existing) {
        // Update existing entity
        existing.confidence = (existing.confidence + entity.confidence) / 2;
        existing.attributes = { ...existing.attributes, ...entity.attributes };
      } else {
        // Add new entity
        this.entities.set(entity.id, entity);
      }
    });
  }

  /**
   * Add a relation between entities
   */
  addRelation(relation: Relation): void {
    this.relations.push(relation);
  }

  /**
   * Query entities by type
   */
  queryEntities(entityType?: EntityType): Entity[] {
    if (!entityType) {
      return Array.from(this.entities.values());
    }

    return Array.from(this.entities.values()).filter(
      e => e.entityType === entityType
    );
  }

  /**
   * Find related entities
   */
  findRelated(entityId: string, maxDepth: number = 2): Set<string> {
    const related = new Set<string>();
    const toExplore: Array<[string, number]> = [[entityId, 0]];

    while (toExplore.length > 0) {
      const [currentId, depth] = toExplore.shift()!;

      if (depth >= maxDepth) {
        continue;
      }

      this.relations.forEach(relation => {
        if (relation.subject === currentId) {
          related.add(relation.object);
          toExplore.push([relation.object, depth + 1]);
        } else if (relation.object === currentId) {
          related.add(relation.subject);
          toExplore.push([relation.subject, depth + 1]);
        }
      });
    }

    return related;
  }

  /**
   * Get entity count
   */
  entityCount(): number {
    return this.entities.size;
  }

  /**
   * Get relation count
   */
  relationCount(): number {
    return this.relations.length;
  }
}
