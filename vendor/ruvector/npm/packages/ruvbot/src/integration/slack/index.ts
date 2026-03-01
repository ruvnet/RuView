/**
 * Slack Integration - Events, Commands, Blocks
 */

export interface SlackIntegration {
  events: SlackEventHandler;
  commands: SlackCommandHandler;
  blocks: BlockBuilder;
}

export interface SlackEventHandler {
  onMessage(handler: (event: SlackMessageEvent) => Promise<void>): void;
  onMention(handler: (event: SlackMentionEvent) => Promise<void>): void;
  onReaction(handler: (event: SlackReactionEvent) => Promise<void>): void;
}

export interface SlackMessageEvent {
  teamId: string;
  channelId: string;
  userId: string;
  text: string;
  threadTs?: string;
  ts: string;
}

export interface SlackMentionEvent extends SlackMessageEvent {
  mentionedUserId: string;
}

export interface SlackReactionEvent {
  teamId: string;
  channelId: string;
  userId: string;
  reaction: string;
  itemTs: string;
  added: boolean;
}

export interface SlackCommandHandler {
  register(command: string, handler: CommandHandler): void;
}

export type CommandHandler = (ctx: CommandContext) => Promise<CommandResult>;

export interface CommandContext {
  command: string;
  text: string;
  userId: string;
  channelId: string;
  teamId: string;
  responseUrl: string;
}

export interface CommandResult {
  public: boolean;
  text?: string;
  blocks?: Block[];
}

export interface BlockBuilder {
  section(text: string): BlockBuilder;
  divider(): BlockBuilder;
  context(...elements: string[]): BlockBuilder;
  actions(actionId: string, buttons: Button[]): BlockBuilder;
  build(): Block[];
}

export interface Block {
  type: string;
  [key: string]: unknown;
}

export interface Button {
  text: string;
  actionId: string;
  value?: string;
  style?: 'primary' | 'danger';
}
