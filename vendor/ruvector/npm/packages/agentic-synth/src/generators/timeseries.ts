/**
 * Time-series data generator
 */

import { BaseGenerator } from './base.js';
import { TimeSeriesOptions, ValidationError } from '../types.js';

export class TimeSeriesGenerator extends BaseGenerator<TimeSeriesOptions> {
  protected generatePrompt(options: TimeSeriesOptions): string {
    const {
      count = 100,
      startDate = new Date(),
      endDate,
      interval = '1h',
      metrics = ['value'],
      trend = 'stable',
      seasonality = false,
      noise = 0.1,
      schema,
      constraints
    } = options;

    const end = endDate || new Date(Date.now() + 24 * 60 * 60 * 1000);

    let prompt = `Generate ${count} time-series data points with the following specifications:

Time Range:
- Start: ${startDate}
- End: ${end}
- Interval: ${interval}

Metrics: ${metrics.join(', ')}

Characteristics:
- Trend: ${trend}
- Seasonality: ${seasonality ? 'Include daily/weekly patterns' : 'No seasonality'}
- Noise level: ${noise * 100}%

`;

    if (schema) {
      prompt += `\nSchema:\n${JSON.stringify(schema, null, 2)}\n`;
    }

    if (constraints) {
      prompt += `\nConstraints:\n${JSON.stringify(constraints, null, 2)}\n`;
    }

    prompt += `
Generate realistic time-series data with timestamps and metric values.
Return the data as a JSON array where each object has:
- timestamp: ISO 8601 formatted date string
- ${metrics.map(m => `${m}: numeric value`).join('\n- ')}

Ensure:
1. Timestamps are evenly spaced according to the interval
2. Values follow the specified trend pattern
3. Noise is applied realistically
4. Seasonality patterns are natural if enabled
5. All values are within reasonable ranges

Return ONLY the JSON array, no additional text.`;

    return prompt;
  }

  protected parseResult(response: string, options: TimeSeriesOptions): unknown[] {
    try {
      // Extract JSON from response
      const jsonMatch = response.match(/\[[\s\S]*\]/);
      if (!jsonMatch) {
        throw new Error('No JSON array found in response');
      }

      const data = JSON.parse(jsonMatch[0]);

      if (!Array.isArray(data)) {
        throw new Error('Response is not an array');
      }

      // Validate and transform data
      return data.map((item, index) => {
        if (typeof item !== 'object' || item === null) {
          throw new ValidationError(`Invalid data item at index ${index}`, { item });
        }

        const record = item as Record<string, unknown>;
        if (!record.timestamp) {
          throw new ValidationError(`Missing timestamp at index ${index}`, { item });
        }

        // Ensure all specified metrics are present
        const metrics = options.metrics || ['value'];
        for (const metric of metrics) {
          if (typeof record[metric] !== 'number') {
            throw new ValidationError(
              `Missing or invalid metric '${metric}' at index ${index}`,
              { item }
            );
          }
        }

        return {
          timestamp: new Date(record.timestamp as string | number | Date).toISOString(),
          ...record
        };
      });
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      throw new ValidationError(`Failed to parse time-series data: ${errorMessage}`, {
        response: response.substring(0, 200),
        error
      });
    }
  }

  /**
   * Generate synthetic time-series with local computation (faster for simple patterns)
   */
  async generateLocal(options: TimeSeriesOptions): Promise<Array<Record<string, unknown>>> {
    const {
      count = 100,
      startDate = new Date(),
      interval = '1h',
      metrics = ['value'],
      trend = 'stable',
      seasonality = false,
      noise = 0.1
    } = options;

    const start = new Date(startDate).getTime();
    const intervalMs = this.parseInterval(interval);
    const data: Array<Record<string, unknown>> = [];

    let baseValue = 100;
    const trendRate = trend === 'up' ? 0.01 : trend === 'down' ? -0.01 : 0;

    for (let i = 0; i < count; i++) {
      const timestamp = new Date(start + i * intervalMs);
      const point: Record<string, unknown> = { timestamp: timestamp.toISOString() };

      for (const metric of metrics) {
        let value = baseValue;

        // Apply trend
        value += baseValue * trendRate * i;

        // Apply seasonality
        if (seasonality) {
          const hourOfDay = timestamp.getHours();
          const dayOfWeek = timestamp.getDay();
          value += Math.sin((hourOfDay / 24) * Math.PI * 2) * baseValue * 0.1;
          value += Math.sin((dayOfWeek / 7) * Math.PI * 2) * baseValue * 0.05;
        }

        // Apply noise
        value += (Math.random() - 0.5) * 2 * baseValue * noise;

        point[metric] = Math.round(value * 100) / 100;
      }

      data.push(point);
    }

    return data;
  }

  private parseInterval(interval: string): number {
    const match = interval.match(/^(\d+)(s|m|h|d)$/);
    if (!match) {
      throw new ValidationError('Invalid interval format', { interval });
    }

    const [, amount, unit] = match;

    // Strict mode: ensure captured groups are defined
    if (!amount || !unit) {
      throw new ValidationError('Invalid interval format: missing amount or unit', { interval, match });
    }

    const multipliers: Record<string, number> = {
      s: 1000,
      m: 60 * 1000,
      h: 60 * 60 * 1000,
      d: 24 * 60 * 60 * 1000
    };

    const multiplier = multipliers[unit];
    if (multiplier === undefined) {
      throw new ValidationError('Invalid interval unit', { interval, unit });
    }

    return parseInt(amount, 10) * multiplier;
  }
}
