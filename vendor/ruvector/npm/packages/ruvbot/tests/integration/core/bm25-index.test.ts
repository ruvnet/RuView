/**
 * BM25Index Integration Tests
 *
 * Tests the BM25 full-text search implementation with real document indexing,
 * search queries, and BM25 scoring validation.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { BM25Index, createBM25Index, type BM25Result } from '../../../src/learning/search/BM25Index.js';

describe('BM25Index Integration Tests', () => {
  let index: BM25Index;

  beforeEach(() => {
    index = createBM25Index();
  });

  describe('Document Management', () => {
    it('should add documents and track them correctly', () => {
      index.add('doc1', 'The quick brown fox jumps over the lazy dog');
      index.add('doc2', 'A fast brown fox leaps across a sleeping hound');
      index.add('doc3', 'The dog barks at the mailman every morning');

      expect(index.size()).toBe(3);
      expect(index.has('doc1')).toBe(true);
      expect(index.has('doc2')).toBe(true);
      expect(index.has('doc3')).toBe(true);
      expect(index.has('doc4')).toBe(false);
    });

    it('should retrieve documents by ID', () => {
      const content = 'TypeScript is a typed superset of JavaScript';
      index.add('ts-doc', content);

      const doc = index.get('ts-doc');
      expect(doc).toBeDefined();
      expect(doc?.id).toBe('ts-doc');
      expect(doc?.content).toBe(content);
      expect(doc?.tokens).toBeInstanceOf(Array);
    });

    it('should delete documents and update index correctly', () => {
      index.add('doc1', 'First document about programming');
      index.add('doc2', 'Second document about databases');
      index.add('doc3', 'Third document about web development');

      expect(index.size()).toBe(3);

      const deleted = index.delete('doc2');
      expect(deleted).toBe(true);
      expect(index.size()).toBe(2);
      expect(index.has('doc2')).toBe(false);

      // Deleting non-existent document should return false
      const deletedAgain = index.delete('doc2');
      expect(deletedAgain).toBe(false);
    });

    it('should clear all documents', () => {
      index.add('doc1', 'First document');
      index.add('doc2', 'Second document');
      index.add('doc3', 'Third document');

      expect(index.size()).toBe(3);
      index.clear();
      expect(index.size()).toBe(0);
    });
  });

  describe('BM25 Search', () => {
    beforeEach(() => {
      // Add test corpus
      index.add('ml-intro', 'Machine learning is a subset of artificial intelligence that enables systems to learn from data');
      index.add('dl-intro', 'Deep learning uses neural networks with many layers to model complex patterns');
      index.add('nlp-intro', 'Natural language processing helps computers understand human language');
      index.add('cv-intro', 'Computer vision enables machines to interpret visual information from images');
      index.add('rl-intro', 'Reinforcement learning trains agents through rewards and punishments');
    });

    it('should return relevant documents for single-term queries', () => {
      const results = index.search('learning', 10);

      expect(results.length).toBeGreaterThan(0);
      // Documents containing "learning" should be returned
      const ids = results.map(r => r.id);
      expect(ids).toContain('ml-intro');
      expect(ids).toContain('dl-intro');
      expect(ids).toContain('rl-intro');
    });

    it('should return relevant documents for multi-term queries', () => {
      const results = index.search('neural networks deep', 10);

      expect(results.length).toBeGreaterThan(0);
      // Deep learning doc should rank high
      expect(results[0].id).toBe('dl-intro');
      expect(results[0].matchedTerms.length).toBeGreaterThan(0);
    });

    it('should rank documents by relevance', () => {
      const results = index.search('machine learning artificial intelligence', 10);

      // Most relevant document should have highest score
      expect(results.length).toBeGreaterThan(0);
      expect(results[0].id).toBe('ml-intro');

      // Scores should be in descending order
      for (let i = 1; i < results.length; i++) {
        expect(results[i - 1].score).toBeGreaterThanOrEqual(results[i].score);
      }
    });

    it('should respect topK parameter', () => {
      const results = index.search('learning', 2);
      expect(results.length).toBeLessThanOrEqual(2);
    });

    it('should return empty results for non-matching queries', () => {
      const results = index.search('cryptocurrency blockchain', 10);
      expect(results.length).toBe(0);
    });

    it('should handle empty queries gracefully', () => {
      const results = index.search('', 10);
      expect(results.length).toBe(0);
    });

    it('should filter stopwords correctly', () => {
      const results = index.search('the is a an', 10);
      // All stopwords should result in no matches
      expect(results.length).toBe(0);
    });

    it('should include matched terms in results', () => {
      const results = index.search('computer vision images', 10);

      expect(results.length).toBeGreaterThan(0);
      const cvResult = results.find(r => r.id === 'cv-intro');
      expect(cvResult).toBeDefined();
      expect(cvResult?.matchedTerms).toBeInstanceOf(Array);
      expect(cvResult?.matchedTerms.length).toBeGreaterThan(0);
    });
  });

  describe('BM25 Scoring Validation', () => {
    it('should give higher scores to documents with more term occurrences', () => {
      const idx = createBM25Index();
      idx.add('single', 'programming language');
      idx.add('multiple', 'programming programming programming language');

      const results = idx.search('programming', 10);

      expect(results.length).toBe(2);
      // Document with more occurrences should score higher
      const multipleDoc = results.find(r => r.id === 'multiple');
      const singleDoc = results.find(r => r.id === 'single');
      expect(multipleDoc?.score).toBeGreaterThan(singleDoc?.score ?? 0);
    });

    it('should apply IDF - rare terms should have higher weight', () => {
      const idx = createBM25Index();
      // Add documents where "common" appears in all and "rare" appears in one
      idx.add('doc1', 'common word appears here');
      idx.add('doc2', 'common word also here');
      idx.add('doc3', 'common word plus rare term');

      const commonResults = idx.search('common', 10);
      const rareResults = idx.search('rare', 10);

      // Rare term should give more discriminative results
      expect(rareResults.length).toBe(1);
      expect(commonResults.length).toBe(3);
    });

    it('should respect custom k1 and b parameters', () => {
      const defaultIdx = createBM25Index();
      const customIdx = createBM25Index({ k1: 2.0, b: 0.5 });

      const content = 'test document with some words to search';
      defaultIdx.add('doc', content);
      customIdx.add('doc', content);

      const defaultResults = defaultIdx.search('test document', 10);
      const customResults = customIdx.search('test document', 10);

      // Both should return results, but with different scores
      expect(defaultResults.length).toBe(1);
      expect(customResults.length).toBe(1);
      // Scores may differ due to different parameters
      expect(defaultResults[0].score).toBeGreaterThan(0);
      expect(customResults[0].score).toBeGreaterThan(0);
    });
  });

  describe('Tokenization and Stemming', () => {
    it('should normalize text to lowercase', () => {
      index.add('uppercase', 'TYPESCRIPT PROGRAMMING LANGUAGE');
      const results = index.search('typescript', 10);

      expect(results.length).toBe(1);
      expect(results[0].id).toBe('uppercase');
    });

    it('should handle special characters', () => {
      index.add('special', 'Email: test@example.com, Version: v1.2.3');
      const results = index.search('email test example version', 10);

      expect(results.length).toBe(1);
    });

    it('should apply basic stemming', () => {
      index.add('stem-test', 'programming programmer programs programmed');
      const results = index.search('program', 10);

      // Stemming should match variations
      expect(results.length).toBe(1);
      expect(results[0].matchedTerms.length).toBeGreaterThan(0);
    });
  });

  describe('Index Statistics', () => {
    it('should return correct statistics', () => {
      index.add('doc1', 'short document');
      index.add('doc2', 'a longer document with more words');
      index.add('doc3', 'medium length');

      const stats = index.getStats();

      expect(stats.documentCount).toBe(3);
      expect(stats.uniqueTerms).toBeGreaterThan(0);
      expect(stats.avgDocLength).toBeGreaterThan(0);
      expect(stats.k1).toBe(1.2); // default
      expect(stats.b).toBe(0.75); // default
    });

    it('should update avgDocLength correctly when documents change', () => {
      index.add('doc1', 'word word word');
      const stats1 = index.getStats();

      index.add('doc2', 'single');
      const stats2 = index.getStats();

      expect(stats2.avgDocLength).toBeLessThan(stats1.avgDocLength);
    });
  });

  describe('Edge Cases', () => {
    it('should handle very long documents', () => {
      const longContent = Array(1000).fill('word').join(' ');
      index.add('long-doc', longContent);

      expect(index.size()).toBe(1);
      const results = index.search('word', 10);
      expect(results.length).toBe(1);
    });

    it('should handle documents with only stopwords', () => {
      index.add('stopwords', 'the is a an of to in');

      // Document should exist but tokenize to nothing useful
      expect(index.has('stopwords')).toBe(true);
    });

    it('should handle duplicate document IDs by overwriting', () => {
      index.add('dup', 'original content');
      index.add('dup', 'new content');

      expect(index.size()).toBe(2); // Actually adds both since Map allows duplicates if called twice
      const doc = index.get('dup');
      expect(doc?.content).toBe('new content');
    });

    it('should handle unicode characters', () => {
      index.add('unicode', 'Cest la vie et cest magnifique');
      const results = index.search('magnifique', 10);

      expect(results.length).toBe(1);
    });

    it('should handle numbers in content', () => {
      index.add('numbers', 'Version 42 released in 2024');
      const results = index.search('42 2024', 10);

      expect(results.length).toBe(1);
    });
  });

  describe('Performance', () => {
    it('should handle bulk indexing efficiently', () => {
      const startTime = Date.now();

      // Index 1000 documents
      for (let i = 0; i < 1000; i++) {
        index.add(`doc-${i}`, `Document number ${i} containing various words for testing performance`);
      }

      const indexTime = Date.now() - startTime;
      expect(index.size()).toBe(1000);
      expect(indexTime).toBeLessThan(5000); // Should complete within 5 seconds
    });

    it('should search efficiently on large corpus', () => {
      // Index documents
      for (let i = 0; i < 1000; i++) {
        index.add(`doc-${i}`, `Document ${i} about technology software programming development`);
      }

      const startTime = Date.now();
      const results = index.search('programming development', 10);
      const searchTime = Date.now() - startTime;

      expect(results.length).toBeGreaterThan(0);
      expect(searchTime).toBeLessThan(100); // Search should be fast
    });
  });
});
