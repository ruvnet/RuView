/**
 * Slack Integration - Integration Tests
 *
 * Tests for Slack message handling, events, and API interactions
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  MockSlackWebClient,
  MockSlackBoltApp,
  createMockSlackClient,
  createMockSlackApp
} from '../../mocks/slack.mock';
import { slackFixtures } from '../../fixtures';

describe('Slack Web Client', () => {
  let client: MockSlackWebClient;

  beforeEach(() => {
    client = createMockSlackClient();
  });

  afterEach(() => {
    client.reset();
  });

  describe('Chat API', () => {
    it('should post message', async () => {
      const response = await client.chat.postMessage({
        channel: 'C12345678',
        text: 'Hello, world!'
      });

      expect(response.ok).toBe(true);
      expect(response.ts).toBeDefined();
      expect(response.channel).toBe('C12345678');
    });

    it('should post message with blocks', async () => {
      const blocks = [
        {
          type: 'section',
          text: { type: 'mrkdwn', text: '*Bold text*' }
        }
      ];

      const response = await client.chat.postMessage({
        channel: 'C12345678',
        text: 'Fallback text',
        blocks
      });

      expect(response.ok).toBe(true);
      expect(client.getMessageLog()).toHaveLength(1);
      expect(client.getMessageLog()[0].blocks).toEqual(blocks);
    });

    it('should post thread reply', async () => {
      const parentTs = '1234567890.123456';

      const response = await client.chat.postMessage({
        channel: 'C12345678',
        text: 'Thread reply',
        thread_ts: parentTs
      });

      expect(response.ok).toBe(true);
      expect(client.getMessageLog()[0].thread_ts).toBe(parentTs);
    });

    it('should update message', async () => {
      const postResponse = await client.chat.postMessage({
        channel: 'C12345678',
        text: 'Original'
      });

      const updateResponse = await client.chat.update({
        channel: 'C12345678',
        ts: postResponse.ts!,
        text: 'Updated'
      });

      expect(updateResponse.ok).toBe(true);
    });

    it('should delete message', async () => {
      const postResponse = await client.chat.postMessage({
        channel: 'C12345678',
        text: 'To delete'
      });

      const deleteResponse = await client.chat.delete({
        channel: 'C12345678',
        ts: postResponse.ts!
      });

      expect(deleteResponse.ok).toBe(true);
    });

    it('should post ephemeral message', async () => {
      const response = await client.chat.postEphemeral({
        channel: 'C12345678',
        user: 'U12345678',
        text: 'Only you can see this'
      });

      expect(response.ok).toBe(true);
    });
  });

  describe('Conversations API', () => {
    it('should get channel info', async () => {
      const response = await client.conversations.info({
        channel: 'C12345678'
      });

      expect(response.ok).toBe(true);
      expect(response.channel?.id).toBe('C12345678');
      expect(response.channel?.name).toBe('general');
    });

    it('should list channel members', async () => {
      const response = await client.conversations.members({
        channel: 'C12345678'
      });

      expect(response.ok).toBe(true);
      expect(response.members).toContain('U12345678');
    });

    it('should get conversation history', async () => {
      // Post some messages first
      await client.chat.postMessage({ channel: 'C12345678', text: 'Message 1' });
      await client.chat.postMessage({ channel: 'C12345678', text: 'Message 2' });

      const response = await client.conversations.history({
        channel: 'C12345678',
        limit: 10
      });

      expect(response.ok).toBe(true);
      expect(response.messages).toHaveLength(2);
    });

    it('should get thread replies', async () => {
      const parentTs = '1234567890.123456';

      await client.chat.postMessage({
        channel: 'C12345678',
        text: 'Reply 1',
        thread_ts: parentTs
      });

      const response = await client.conversations.replies({
        channel: 'C12345678',
        ts: parentTs
      });

      expect(response.ok).toBe(true);
      expect(response.messages).toHaveLength(1);
    });
  });

  describe('Users API', () => {
    it('should get user info', async () => {
      const response = await client.users.info({
        user: 'U12345678'
      });

      expect(response.ok).toBe(true);
      expect(response.user?.id).toBe('U12345678');
      expect(response.user?.name).toBe('testuser');
      expect(response.user?.is_bot).toBe(false);
    });

    it('should list users', async () => {
      const response = await client.users.list();

      expect(response.ok).toBe(true);
      expect(response.members.length).toBeGreaterThan(0);
    });
  });

  describe('Reactions API', () => {
    it('should add reaction', async () => {
      const response = await client.reactions.add({
        channel: 'C12345678',
        timestamp: '1234567890.123456',
        name: 'thumbsup'
      });

      expect(response.ok).toBe(true);
      expect(client.getReactions('C12345678', '1234567890.123456')).toContain('thumbsup');
    });

    it('should remove reaction', async () => {
      await client.reactions.add({
        channel: 'C12345678',
        timestamp: '1234567890.123456',
        name: 'thumbsup'
      });

      const response = await client.reactions.remove({
        channel: 'C12345678',
        timestamp: '1234567890.123456',
        name: 'thumbsup'
      });

      expect(response.ok).toBe(true);
      expect(client.getReactions('C12345678', '1234567890.123456')).not.toContain('thumbsup');
    });
  });

  describe('Files API', () => {
    it('should upload file', async () => {
      const response = await client.files.upload({
        channels: 'C12345678',
        content: 'console.log("Hello");',
        filename: 'script.js'
      });

      expect(response.ok).toBe(true);
      expect(response.file).toBeDefined();
    });
  });

  describe('Auth API', () => {
    it('should verify auth', async () => {
      const response = await client.auth.test();

      expect(response.ok).toBe(true);
      expect(response.user_id).toBe('U_BOT');
      expect(response.team_id).toBe('T12345678');
    });
  });
});

describe('Slack Bolt App', () => {
  let app: MockSlackBoltApp;

  beforeEach(() => {
    app = createMockSlackApp();
  });

  afterEach(() => {
    app.reset();
  });

  describe('Message Handlers', () => {
    it('should handle message with string pattern', async () => {
      const handler = vi.fn(async ({ say }) => {
        await say({ channel: 'C12345678', text: 'Response' });
      });

      app.message('hello', handler);

      await app.processMessage({
        text: 'hello world',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      expect(handler).toHaveBeenCalled();
    });

    it('should handle message with regex pattern', async () => {
      const handler = vi.fn();

      app.message(/help/i, handler);

      await app.processMessage({
        text: 'I need HELP',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      expect(handler).toHaveBeenCalled();
    });

    it('should not trigger handler for non-matching message', async () => {
      const handler = vi.fn();

      app.message('specific', handler);

      await app.processMessage({
        text: 'other message',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      expect(handler).not.toHaveBeenCalled();
    });

    it('should provide say function to handler', async () => {
      app.message('test', async ({ say }) => {
        await say({ channel: 'C12345678', text: 'Reply' });
      });

      await app.processMessage({
        text: 'test',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.123456'
      });

      expect(app.client.getMessageLog()).toHaveLength(1);
      expect(app.client.getMessageLog()[0].text).toBe('Reply');
    });
  });

  describe('Action Handlers', () => {
    it('should handle button action', async () => {
      const handler = vi.fn(async ({ ack }) => {
        await ack();
      });

      app.action('button_click', handler);

      await app.processAction('button_click', {
        user: { id: 'U12345678' },
        channel: { id: 'C12345678' }
      });

      expect(handler).toHaveBeenCalled();
    });
  });

  describe('Command Handlers', () => {
    it('should handle slash command', async () => {
      const handler = vi.fn(async ({ ack, respond }) => {
        await ack();
        await respond({ text: 'Command received' });
      });

      app.command('/ruvbot', handler);

      await app.processCommand('/ruvbot', {
        text: 'help',
        user_id: 'U12345678',
        channel_id: 'C12345678'
      });

      expect(handler).toHaveBeenCalled();
    });
  });

  describe('Event Handlers', () => {
    it('should handle app_mention event', async () => {
      const handler = vi.fn();

      app.event('app_mention', handler);

      // Simulate event through internal handler
      const events = (app as any).eventsHandler;
      await events.emit('app_mention', slackFixtures.appMentionEvent);

      expect(handler).toHaveBeenCalled();
    });
  });

  describe('Lifecycle', () => {
    it('should start app', async () => {
      await expect(app.start(3000)).resolves.not.toThrow();
    });

    it('should stop app', async () => {
      await expect(app.stop()).resolves.not.toThrow();
    });

    it('should reset app state', () => {
      app.message('test', vi.fn());
      app.reset();

      // After reset, handlers should be cleared
      expect(app.client.getMessageLog()).toHaveLength(0);
    });
  });
});

describe('Slack Event Processing', () => {
  let app: MockSlackBoltApp;

  beforeEach(() => {
    app = createMockSlackApp();
  });

  afterEach(() => {
    app.reset();
  });

  describe('Message Flow', () => {
    it('should process complete message flow', async () => {
      const messagesReceived: string[] = [];
      const repliesSent: string[] = [];

      app.message(/.*/, async ({ message, say }) => {
        messagesReceived.push((message as any).text);
        await say({
          channel: (message as any).channel,
          text: `Received: ${(message as any).text}`,
          thread_ts: (message as any).ts
        });
        repliesSent.push(`Received: ${(message as any).text}`);
      });

      // Simulate conversation
      await app.processMessage({
        text: 'Hello bot',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.111111'
      });

      await app.processMessage({
        text: 'How are you?',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.222222'
      });

      expect(messagesReceived).toEqual(['Hello bot', 'How are you?']);
      expect(repliesSent).toHaveLength(2);
    });

    it('should handle thread conversations', async () => {
      const threadMessages: string[] = [];

      app.message(/.*/, async ({ message }) => {
        if ((message as any).thread_ts) {
          threadMessages.push((message as any).text);
        }
      });

      const parentTs = '1234567890.000000';

      await app.processMessage({
        text: 'Parent message',
        channel: 'C12345678',
        user: 'U12345678',
        ts: parentTs
      });

      await app.processMessage({
        text: 'Reply 1',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.111111',
        thread_ts: parentTs
      });

      await app.processMessage({
        text: 'Reply 2',
        channel: 'C12345678',
        user: 'U12345678',
        ts: '1234567890.222222',
        thread_ts: parentTs
      });

      expect(threadMessages).toEqual(['Reply 1', 'Reply 2']);
    });
  });

  describe('Multi-channel Handling', () => {
    it('should handle messages from different channels', async () => {
      const channelMessages: Record<string, string[]> = {};

      app.message(/.*/, async ({ message }) => {
        const channel = (message as any).channel;
        if (!channelMessages[channel]) {
          channelMessages[channel] = [];
        }
        channelMessages[channel].push((message as any).text);
      });

      await app.processMessage({
        text: 'Channel 1 message',
        channel: 'C11111111',
        user: 'U12345678',
        ts: '1234567890.111111'
      });

      await app.processMessage({
        text: 'Channel 2 message',
        channel: 'C22222222',
        user: 'U12345678',
        ts: '1234567890.222222'
      });

      expect(channelMessages['C11111111']).toEqual(['Channel 1 message']);
      expect(channelMessages['C22222222']).toEqual(['Channel 2 message']);
    });
  });

  describe('User Interactions', () => {
    it('should track user information in context', async () => {
      let capturedUserId: string | undefined;

      app.message(/.*/, async ({ message }) => {
        capturedUserId = (message as any).user;
      });

      await app.processMessage({
        text: 'Test',
        channel: 'C12345678',
        user: 'U_SPECIFIC_USER',
        ts: '1234567890.111111'
      });

      expect(capturedUserId).toBe('U_SPECIFIC_USER');
    });
  });
});

describe('Slack Response Formatting', () => {
  let client: MockSlackWebClient;

  beforeEach(() => {
    client = createMockSlackClient();
  });

  describe('Block Formatting', () => {
    it('should format code blocks', async () => {
      const codeBlock = {
        type: 'section',
        text: {
          type: 'mrkdwn',
          text: '```javascript\nconsole.log("Hello");\n```'
        }
      };

      await client.chat.postMessage({
        channel: 'C12345678',
        text: 'Code example',
        blocks: [codeBlock]
      });

      const log = client.getMessageLog();
      expect(log[0].blocks![0]).toEqual(codeBlock);
    });

    it('should format interactive buttons', async () => {
      const buttonBlock = {
        type: 'actions',
        elements: [
          {
            type: 'button',
            text: { type: 'plain_text', text: 'Approve' },
            style: 'primary',
            action_id: 'approve'
          },
          {
            type: 'button',
            text: { type: 'plain_text', text: 'Reject' },
            style: 'danger',
            action_id: 'reject'
          }
        ]
      };

      await client.chat.postMessage({
        channel: 'C12345678',
        text: 'Please review',
        blocks: [buttonBlock]
      });

      const log = client.getMessageLog();
      expect(log[0].blocks![0]).toEqual(buttonBlock);
    });
  });
});
