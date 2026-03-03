/**
 * Matrix utilities for diagonal dominance, conditioning, and validation
 */

class MatrixUtils {
  /**
   * Check if matrix has proper diagonal elements
   */
  static validateDiagonalElements(matrix) {
    const issues = [];
    const missingDiagonals = [];
    const smallDiagonals = [];

    if (matrix.format === 'dense') {
      for (let i = 0; i < matrix.rows; i++) {
        const diagonal = matrix.data[i][i];
        if (diagonal === undefined || diagonal === null) {
          missingDiagonals.push(i);
        } else if (Math.abs(diagonal) < 1e-14) {
          smallDiagonals.push({ index: i, value: diagonal });
        }
      }
    } else if (matrix.format === 'coo') {
      const diagonalElements = new Map();

      // Collect all diagonal elements
      for (let k = 0; k < matrix.data.values.length; k++) {
        const row = matrix.data.rowIndices[k];
        const col = matrix.data.colIndices[k];

        if (row === col) {
          diagonalElements.set(row, matrix.data.values[k]);
        }
      }

      // Check for missing or small diagonals
      for (let i = 0; i < matrix.rows; i++) {
        if (!diagonalElements.has(i)) {
          missingDiagonals.push(i);
        } else {
          const diagonal = diagonalElements.get(i);
          if (Math.abs(diagonal) < 1e-14) {
            smallDiagonals.push({ index: i, value: diagonal });
          }
        }
      }
    }

    return {
      valid: missingDiagonals.length === 0 && smallDiagonals.length === 0,
      missingDiagonals,
      smallDiagonals,
      issues
    };
  }

  /**
   * Fix matrix by ensuring diagonal dominance
   */
  static ensureDiagonalDominance(matrix, options = {}) {
    const {
      strategy = 'rowsum_plus_one',
      minDiagonalValue = 1e-12,
      verbose = false
    } = options;

    if (matrix.format === 'dense') {
      return this.ensureDiagonalDominanceDense(matrix, strategy, minDiagonalValue, verbose);
    } else if (matrix.format === 'coo') {
      return this.ensureDiagonalDominanceCOO(matrix, strategy, minDiagonalValue, verbose);
    } else {
      throw new Error(`Unsupported matrix format: ${matrix.format}`);
    }
  }

  static ensureDiagonalDominanceDense(matrix, strategy, minDiagonalValue, verbose) {
    const fixedMatrix = {
      ...matrix,
      data: matrix.data.map(row => [...row]) // Deep copy
    };

    const fixes = [];

    for (let i = 0; i < matrix.rows; i++) {
      const row = fixedMatrix.data[i];
      let rowSum = 0;
      let currentDiagonal = row[i];

      // Calculate row sum (excluding diagonal)
      for (let j = 0; j < matrix.cols; j++) {
        if (i !== j) {
          rowSum += Math.abs(row[j]);
        }
      }

      let newDiagonal;
      switch (strategy) {
        case 'rowsum_plus_one':
          newDiagonal = rowSum + Math.abs(rowSum) + 1;
          break;
        case 'rowsum_times_1_5':
          newDiagonal = Math.max(rowSum * 1.5, minDiagonalValue);
          break;
        case 'preserve_sign':
          const sign = currentDiagonal >= 0 ? 1 : -1;
          newDiagonal = sign * Math.max(rowSum + 1, Math.abs(currentDiagonal), minDiagonalValue);
          break;
        default:
          newDiagonal = Math.max(rowSum + 1, minDiagonalValue);
      }

      if (Math.abs(currentDiagonal) < minDiagonalValue ||
          Math.abs(currentDiagonal) < rowSum) {

        fixes.push({
          row: i,
          oldValue: currentDiagonal,
          newValue: newDiagonal,
          rowSum: rowSum
        });

        fixedMatrix.data[i][i] = newDiagonal;

        if (verbose) {
          console.log(`Fixed diagonal[${i}]: ${currentDiagonal} → ${newDiagonal} (row sum: ${rowSum})`);
        }
      }
    }

    return { matrix: fixedMatrix, fixes };
  }

  static ensureDiagonalDominanceCOO(matrix, strategy, minDiagonalValue, verbose) {
    const values = [...matrix.data.values];
    const rowIndices = [...matrix.data.rowIndices];
    const colIndices = [...matrix.data.colIndices];
    const fixes = [];

    // Find existing diagonal elements and compute row sums
    const diagonalIndices = new Map();
    const rowSums = new Array(matrix.rows).fill(0);

    for (let k = 0; k < values.length; k++) {
      const row = rowIndices[k];
      const col = colIndices[k];
      const val = values[k];

      if (row === col) {
        diagonalIndices.set(row, k);
      } else {
        rowSums[row] += Math.abs(val);
      }
    }

    // Fix or add diagonal elements
    for (let i = 0; i < matrix.rows; i++) {
      const rowSum = rowSums[i];
      let newDiagonal;

      switch (strategy) {
        case 'rowsum_plus_one':
          newDiagonal = rowSum + Math.abs(rowSum) + 1;
          break;
        case 'rowsum_times_1_5':
          newDiagonal = Math.max(rowSum * 1.5, minDiagonalValue);
          break;
        default:
          newDiagonal = Math.max(rowSum + 1, minDiagonalValue);
      }

      if (diagonalIndices.has(i)) {
        // Update existing diagonal
        const k = diagonalIndices.get(i);
        const oldValue = values[k];

        if (Math.abs(oldValue) < minDiagonalValue || Math.abs(oldValue) < rowSum) {
          fixes.push({
            row: i,
            oldValue: oldValue,
            newValue: newDiagonal,
            rowSum: rowSum
          });

          values[k] = newDiagonal;

          if (verbose) {
            console.log(`Fixed diagonal[${i}]: ${oldValue} → ${newDiagonal} (row sum: ${rowSum})`);
          }
        }
      } else {
        // Add missing diagonal
        fixes.push({
          row: i,
          oldValue: 0,
          newValue: newDiagonal,
          rowSum: rowSum
        });

        values.push(newDiagonal);
        rowIndices.push(i);
        colIndices.push(i);

        if (verbose) {
          console.log(`Added diagonal[${i}]: ${newDiagonal} (row sum: ${rowSum})`);
        }
      }
    }

    const fixedMatrix = {
      ...matrix,
      entries: values.length,
      data: {
        values,
        rowIndices,
        colIndices
      }
    };

    return { matrix: fixedMatrix, fixes };
  }

  /**
   * Calculate matrix condition metrics
   */
  static analyzeConditioning(matrix) {
    const validation = this.validateDiagonalElements(matrix);

    let diagonalDominanceRatio = 0;
    let minDiagonalMagnitude = Infinity;
    let maxOffDiagonalSum = 0;

    if (matrix.format === 'dense') {
      for (let i = 0; i < matrix.rows; i++) {
        const diagonal = Math.abs(matrix.data[i][i]);
        minDiagonalMagnitude = Math.min(minDiagonalMagnitude, diagonal);

        let offDiagonalSum = 0;
        for (let j = 0; j < matrix.cols; j++) {
          if (i !== j) {
            offDiagonalSum += Math.abs(matrix.data[i][j]);
          }
        }

        maxOffDiagonalSum = Math.max(maxOffDiagonalSum, offDiagonalSum);
        if (offDiagonalSum > 0) {
          diagonalDominanceRatio = Math.max(diagonalDominanceRatio, offDiagonalSum / diagonal);
        }
      }
    } else if (matrix.format === 'coo') {
      const diagonals = new Map();
      const rowSums = new Array(matrix.rows).fill(0);

      for (let k = 0; k < matrix.data.values.length; k++) {
        const row = matrix.data.rowIndices[k];
        const col = matrix.data.colIndices[k];
        const val = Math.abs(matrix.data.values[k]);

        if (row === col) {
          diagonals.set(row, val);
        } else {
          rowSums[row] += val;
        }
      }

      for (let i = 0; i < matrix.rows; i++) {
        const diagonal = diagonals.get(i) || 0;
        const offDiagonalSum = rowSums[i];

        if (diagonal > 0) {
          minDiagonalMagnitude = Math.min(minDiagonalMagnitude, diagonal);
          if (offDiagonalSum > 0) {
            diagonalDominanceRatio = Math.max(diagonalDominanceRatio, offDiagonalSum / diagonal);
          }
        }

        maxOffDiagonalSum = Math.max(maxOffDiagonalSum, offDiagonalSum);
      }
    }

    // Determine conditioning quality
    let conditioningGrade;
    let recommendations = [];

    if (validation.missingDiagonals.length > 0) {
      conditioningGrade = 'F';
      recommendations.push('Add missing diagonal elements');
    } else if (validation.smallDiagonals.length > 0) {
      conditioningGrade = 'D';
      recommendations.push('Increase small diagonal elements');
    } else if (diagonalDominanceRatio > 2.0) {
      conditioningGrade = 'C';
      recommendations.push('Improve diagonal dominance');
    } else if (diagonalDominanceRatio > 1.0) {
      conditioningGrade = 'B';
      recommendations.push('Consider preconditioning for better convergence');
    } else {
      conditioningGrade = 'A';
      recommendations.push('Matrix is well-conditioned');
    }

    return {
      validation,
      diagonalDominanceRatio,
      minDiagonalMagnitude: minDiagonalMagnitude === Infinity ? 0 : minDiagonalMagnitude,
      maxOffDiagonalSum,
      conditioningGrade,
      recommendations,
      isDiagonallyDominant: diagonalDominanceRatio <= 1.0,
      isWellConditioned: conditioningGrade === 'A' || conditioningGrade === 'B'
    };
  }

  /**
   * Check if matrix is symmetric (required for CG)
   */
  static isSymmetric(matrix, tolerance = 1e-12) {
    if (matrix.rows !== matrix.cols) return false;

    if (matrix.format === 'dense') {
      for (let i = 0; i < matrix.rows; i++) {
        for (let j = 0; j < matrix.cols; j++) {
          if (Math.abs(matrix.data[i][j] - matrix.data[j][i]) > tolerance) {
            return false;
          }
        }
      }
      return true;
    } else if (matrix.format === 'coo') {
      // Build a map of (i,j) -> value and check symmetry
      const values = new Map();

      for (let k = 0; k < matrix.data.values.length; k++) {
        const i = matrix.data.rowIndices[k];
        const j = matrix.data.colIndices[k];
        const val = matrix.data.values[k];

        const key = `${i},${j}`;
        values.set(key, val);
      }

      for (let k = 0; k < matrix.data.values.length; k++) {
        const i = matrix.data.rowIndices[k];
        const j = matrix.data.colIndices[k];
        const val = matrix.data.values[k];

        const symmetricKey = `${j},${i}`;
        const symmetricVal = values.get(symmetricKey) || 0;

        if (Math.abs(val - symmetricVal) > tolerance) {
          return false;
        }
      }
      return true;
    }

    return false;
  }

  /**
   * Generate a symmetric positive definite matrix (suitable for CG)
   */
  static generateSymmetricPositiveDefiniteMatrix(size, sparsity, options = {}) {
    const {
      diagonalStrategy = 'rowsum_plus_one',
      offDiagonalRange = [-0.3, 0.3],
      ensurePositiveDefinite = true
    } = options;

    const values = [];
    const rowIndices = [];
    const colIndices = [];

    // Generate lower triangular part and mirror to upper
    const numOffDiagonal = Math.floor(size * size * sparsity / 2) - size;

    const offDiagonalPairs = new Set();

    // Add random off-diagonal entries (lower triangular)
    for (let count = 0; count < numOffDiagonal; count++) {
      let i, j;
      do {
        i = Math.floor(Math.random() * size);
        j = Math.floor(Math.random() * size);
      } while (i <= j || offDiagonalPairs.has(`${i},${j}`));

      offDiagonalPairs.add(`${i},${j}`);

      const range = offDiagonalRange[1] - offDiagonalRange[0];
      const value = offDiagonalRange[0] + Math.random() * range;

      // Add both (i,j) and (j,i) for symmetry
      values.push(value, value);
      rowIndices.push(i, j);
      colIndices.push(j, i);
    }

    // Calculate row sums for diagonal dominance
    const rowSums = new Array(size).fill(0);
    for (let k = 0; k < values.length; k++) {
      const row = rowIndices[k];
      rowSums[row] += Math.abs(values[k]);
    }

    // Add diagonal entries to ensure positive definiteness
    for (let i = 0; i < size; i++) {
      const rowSum = rowSums[i];
      let diagonal;

      if (ensurePositiveDefinite) {
        // Ensure diagonal dominance for positive definiteness
        diagonal = Math.max(rowSum * 1.2 + 1, 1);
      } else {
        switch (diagonalStrategy) {
          case 'rowsum_plus_one':
            diagonal = rowSum + Math.abs(rowSum) + 1;
            break;
          case 'rowsum_times_2':
            diagonal = Math.max(rowSum * 2, 1);
            break;
          default:
            diagonal = Math.max(rowSum + 1, 1);
        }
      }

      rowIndices.push(i);
      colIndices.push(i);
      values.push(diagonal);
    }

    return {
      rows: size,
      cols: size,
      entries: values.length,
      format: 'coo',
      data: { values, rowIndices, colIndices }
    };
  }

  /**
   * Generate a well-conditioned sparse matrix
   */
  static generateWellConditionedSparseMatrix(size, sparsity, options = {}) {
    const {
      diagonalStrategy = 'rowsum_plus_one',
      offDiagonalRange = [-0.5, 0.5],
      ensureDominance = true,
      seed = null
    } = options;

    // Set random seed if provided
    if (seed !== null) {
      // Simple linear congruential generator for reproducibility
      let rng = seed;
      Math.random = () => {
        rng = (rng * 1664525 + 1013904223) % 4294967296;
        return rng / 4294967296;
      };
    }

    const values = [];
    const rowIndices = [];
    const colIndices = [];
    const numOffDiagonal = Math.floor(size * size * sparsity) - size;

    // Add random off-diagonal entries first
    for (let i = 0; i < numOffDiagonal; i++) {
      const row = Math.floor(Math.random() * size);
      const col = Math.floor(Math.random() * size);

      if (row !== col) {
        rowIndices.push(row);
        colIndices.push(col);
        const range = offDiagonalRange[1] - offDiagonalRange[0];
        values.push(offDiagonalRange[0] + Math.random() * range);
      }
    }

    // Calculate row sums and add diagonal entries
    const rowSums = new Array(size).fill(0);
    for (let k = 0; k < values.length; k++) {
      const row = rowIndices[k];
      rowSums[row] += Math.abs(values[k]);
    }

    for (let i = 0; i < size; i++) {
      const rowSum = rowSums[i];
      let diagonal;

      switch (diagonalStrategy) {
        case 'rowsum_plus_one':
          diagonal = rowSum + Math.abs(rowSum) + 1;
          break;
        case 'rowsum_times_2':
          diagonal = Math.max(rowSum * 2, 1);
          break;
        case 'fixed_value':
          diagonal = 2 + Math.random();
          break;
        default:
          diagonal = Math.max(rowSum + 1, 1);
      }

      rowIndices.push(i);
      colIndices.push(i);
      values.push(diagonal);
    }

    let matrix = {
      rows: size,
      cols: size,
      entries: values.length,
      format: 'coo',
      data: { values, rowIndices, colIndices }
    };

    // Ensure diagonal dominance if requested
    if (ensureDominance) {
      const result = this.ensureDiagonalDominance(matrix, {
        strategy: diagonalStrategy,
        verbose: false
      });
      matrix = result.matrix;
    }

    return matrix;
  }
}

module.exports = { MatrixUtils };