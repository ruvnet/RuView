"use strict";
/**
 * RuvBot CLI - Channels Command
 *
 * Setup and manage channel integrations (Slack, Discord, Telegram, Webhooks).
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.createChannelsCommand = createChannelsCommand;
exports.createWebhooksCommand = createWebhooksCommand;
const commander_1 = require("commander");
const chalk_1 = __importDefault(require("chalk"));
function createChannelsCommand() {
    const channels = new commander_1.Command('channels')
        .alias('ch')
        .description('Manage channel integrations');
    // List channels
    channels
        .command('list')
        .alias('ls')
        .description('List available channel integrations')
        .option('--json', 'Output as JSON')
        .action((options) => {
        const channelList = [
            {
                name: 'slack',
                description: 'Slack workspace integration via Bolt SDK',
                package: '@slack/bolt',
                status: 'available',
            },
            {
                name: 'discord',
                description: 'Discord server integration via discord.js',
                package: 'discord.js',
                status: 'available',
            },
            {
                name: 'telegram',
                description: 'Telegram bot integration via Telegraf',
                package: 'telegraf',
                status: 'available',
            },
            {
                name: 'webhook',
                description: 'Generic webhook endpoint for custom integrations',
                package: 'built-in',
                status: 'available',
            },
        ];
        if (options.json) {
            console.log(JSON.stringify(channelList, null, 2));
            return;
        }
        console.log(chalk_1.default.bold('\n📡 Available Channel Integrations\n'));
        console.log('─'.repeat(60));
        for (const ch of channelList) {
            const icon = getChannelIcon(ch.name);
            console.log(`${icon} ${chalk_1.default.cyan(ch.name.padEnd(12))} ${ch.description}`);
            console.log(`   Package: ${chalk_1.default.gray(ch.package)}`);
            console.log();
        }
        console.log('─'.repeat(60));
        console.log(chalk_1.default.gray('\nRun `ruvbot channels setup <channel>` for setup instructions'));
    });
    // Setup channel
    channels
        .command('setup <channel>')
        .description('Show setup instructions for a channel')
        .action((channel) => {
        const normalizedChannel = channel.toLowerCase();
        switch (normalizedChannel) {
            case 'slack':
                printSlackSetup();
                break;
            case 'discord':
                printDiscordSetup();
                break;
            case 'telegram':
                printTelegramSetup();
                break;
            case 'webhook':
            case 'webhooks':
                printWebhookSetup();
                break;
            default:
                console.error(chalk_1.default.red(`Unknown channel: ${channel}`));
                console.log('\nAvailable channels: slack, discord, telegram, webhook');
                process.exit(1);
        }
    });
    // Test channel connection
    channels
        .command('test <channel>')
        .description('Test channel connection')
        .action(async (channel) => {
        const normalizedChannel = channel.toLowerCase();
        console.log(chalk_1.default.cyan(`\nTesting ${normalizedChannel} connection...`));
        const envVars = getRequiredEnvVars(normalizedChannel);
        const missing = envVars.filter((v) => !process.env[v]);
        if (missing.length > 0) {
            console.log(chalk_1.default.red('\n✗ Missing environment variables:'));
            missing.forEach((v) => console.log(chalk_1.default.red(`  - ${v}`)));
            console.log(chalk_1.default.gray(`\nRun 'ruvbot channels setup ${normalizedChannel}' for instructions`));
            process.exit(1);
        }
        console.log(chalk_1.default.green('✓ All required environment variables are set'));
        console.log(chalk_1.default.gray('\nStart the bot with:'));
        console.log(chalk_1.default.cyan(`  ruvbot start --channel ${normalizedChannel}`));
    });
    return channels;
}
function getChannelIcon(channel) {
    const icons = {
        slack: '💬',
        discord: '🎮',
        telegram: '✈️',
        webhook: '🔗',
    };
    return icons[channel] || '📡';
}
function getRequiredEnvVars(channel) {
    switch (channel) {
        case 'slack':
            return ['SLACK_BOT_TOKEN', 'SLACK_SIGNING_SECRET', 'SLACK_APP_TOKEN'];
        case 'discord':
            return ['DISCORD_TOKEN', 'DISCORD_CLIENT_ID'];
        case 'telegram':
            return ['TELEGRAM_BOT_TOKEN'];
        case 'webhook':
            return [];
        default:
            return [];
    }
}
function printSlackSetup() {
    console.log(chalk_1.default.bold('\n💬 Slack Integration Setup\n'));
    console.log('═'.repeat(60));
    console.log(chalk_1.default.bold('\n📋 Step 1: Create a Slack App\n'));
    console.log('  1. Go to: ' + chalk_1.default.cyan('https://api.slack.com/apps'));
    console.log('  2. Click "Create New App" → "From Scratch"');
    console.log('  3. Name your app (e.g., "RuvBot") and select workspace');
    console.log(chalk_1.default.bold('\n🔐 Step 2: Configure Bot Permissions\n'));
    console.log('  Navigate to OAuth & Permissions and add these Bot Token Scopes:');
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log('  • app_mentions:read    - Receive @mentions');
    console.log('  • chat:write           - Send messages');
    console.log('  • channels:history     - Read channel messages');
    console.log('  • im:history           - Read direct messages');
    console.log('  • reactions:write      - Add reactions');
    console.log('  • files:read           - Access shared files');
    console.log(chalk_1.default.bold('\n⚡ Step 3: Enable Socket Mode\n'));
    console.log('  1. Go to Socket Mode → Enable');
    console.log('  2. Create App-Level Token with ' + chalk_1.default.cyan('connections:write') + ' scope');
    console.log('  3. Save the ' + chalk_1.default.yellow('xapp-...') + ' token');
    console.log(chalk_1.default.bold('\n📦 Step 4: Install & Get Tokens\n'));
    console.log('  1. Go to Install App → Install to Workspace');
    console.log('  2. Copy Bot User OAuth Token: ' + chalk_1.default.yellow('xoxb-...'));
    console.log('  3. Copy Signing Secret from Basic Information');
    console.log(chalk_1.default.bold('\n🔧 Step 5: Configure Environment\n'));
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log(chalk_1.default.cyan('  export SLACK_BOT_TOKEN="xoxb-your-bot-token"'));
    console.log(chalk_1.default.cyan('  export SLACK_SIGNING_SECRET="your-signing-secret"'));
    console.log(chalk_1.default.cyan('  export SLACK_APP_TOKEN="xapp-your-app-token"'));
    console.log(chalk_1.default.bold('\n🚀 Step 6: Start RuvBot\n'));
    console.log(chalk_1.default.cyan('  ruvbot start --channel slack'));
    console.log(chalk_1.default.bold('\n🌐 Webhook Mode (for Cloud Run)\n'));
    console.log('  For serverless deployments, use webhook instead of Socket Mode:');
    console.log('  1. Disable Socket Mode');
    console.log('  2. Go to Event Subscriptions → Enable');
    console.log('  3. Set Request URL: ' + chalk_1.default.cyan('https://your-ruvbot.run.app/slack/events'));
    console.log('  4. Subscribe to: message.channels, message.im, app_mention');
    console.log('\n' + '═'.repeat(60));
    console.log(chalk_1.default.gray('Install optional dependency: npm install @slack/bolt @slack/web-api\n'));
}
function printDiscordSetup() {
    console.log(chalk_1.default.bold('\n🎮 Discord Integration Setup\n'));
    console.log('═'.repeat(60));
    console.log(chalk_1.default.bold('\n📋 Step 1: Create a Discord Application\n'));
    console.log('  1. Go to: ' + chalk_1.default.cyan('https://discord.com/developers/applications'));
    console.log('  2. Click "New Application" and name it');
    console.log(chalk_1.default.bold('\n🤖 Step 2: Create a Bot\n'));
    console.log('  1. Go to Bot section → Add Bot');
    console.log('  2. Enable Privileged Gateway Intents:');
    console.log(chalk_1.default.green('     ✓ MESSAGE CONTENT INTENT'));
    console.log(chalk_1.default.green('     ✓ SERVER MEMBERS INTENT'));
    console.log('  3. Click "Reset Token" and copy the bot token');
    console.log(chalk_1.default.bold('\n🆔 Step 3: Get Application IDs\n'));
    console.log('  1. Copy Application ID from General Information');
    console.log('  2. Right-click your server → Copy Server ID (for testing)');
    console.log(chalk_1.default.bold('\n📨 Step 4: Invite Bot to Server\n'));
    console.log('  1. Go to OAuth2 → URL Generator');
    console.log('  2. Select scopes: ' + chalk_1.default.cyan('bot, applications.commands'));
    console.log('  3. Select permissions:');
    console.log('     • Send Messages');
    console.log('     • Read Message History');
    console.log('     • Add Reactions');
    console.log('     • Use Slash Commands');
    console.log('  4. Open the generated URL to invite the bot');
    console.log(chalk_1.default.bold('\n🔧 Step 5: Configure Environment\n'));
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log(chalk_1.default.cyan('  export DISCORD_TOKEN="your-bot-token"'));
    console.log(chalk_1.default.cyan('  export DISCORD_CLIENT_ID="your-application-id"'));
    console.log(chalk_1.default.cyan('  export DISCORD_GUILD_ID="your-server-id"  # Optional'));
    console.log(chalk_1.default.bold('\n🚀 Step 6: Start RuvBot\n'));
    console.log(chalk_1.default.cyan('  ruvbot start --channel discord'));
    console.log('\n' + '═'.repeat(60));
    console.log(chalk_1.default.gray('Install optional dependency: npm install discord.js\n'));
}
function printTelegramSetup() {
    console.log(chalk_1.default.bold('\n✈️ Telegram Integration Setup\n'));
    console.log('═'.repeat(60));
    console.log(chalk_1.default.bold('\n📋 Step 1: Create a Bot with BotFather\n'));
    console.log('  1. Open Telegram and search for ' + chalk_1.default.cyan('@BotFather'));
    console.log('  2. Send ' + chalk_1.default.cyan('/newbot') + ' command');
    console.log('  3. Follow prompts to name your bot');
    console.log('  4. Copy the HTTP API token (format: ' + chalk_1.default.yellow('123456789:ABC-DEF...') + ')');
    console.log(chalk_1.default.bold('\n🔧 Step 2: Configure Environment\n'));
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log(chalk_1.default.cyan('  export TELEGRAM_BOT_TOKEN="your-bot-token"'));
    console.log(chalk_1.default.bold('\n🚀 Step 3: Start RuvBot (Polling Mode)\n'));
    console.log(chalk_1.default.cyan('  ruvbot start --channel telegram'));
    console.log(chalk_1.default.bold('\n🌐 Webhook Mode (for Production/Cloud Run)\n'));
    console.log('  For serverless deployments, use webhook mode:');
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log(chalk_1.default.cyan('  export TELEGRAM_BOT_TOKEN="your-bot-token"'));
    console.log(chalk_1.default.cyan('  export TELEGRAM_WEBHOOK_URL="https://your-ruvbot.run.app/telegram/webhook"'));
    console.log(chalk_1.default.bold('\n📱 Step 4: Test Your Bot\n'));
    console.log('  1. Search for your bot by username in Telegram');
    console.log('  2. Start a chat and send ' + chalk_1.default.cyan('/start'));
    console.log('  3. Send messages to interact with RuvBot');
    console.log(chalk_1.default.bold('\n⚙️ Optional: Set Bot Commands\n'));
    console.log('  Send to @BotFather:');
    console.log(chalk_1.default.cyan('  /setcommands'));
    console.log('  Then paste:');
    console.log(chalk_1.default.gray('  start - Start the bot'));
    console.log(chalk_1.default.gray('  help - Show help message'));
    console.log(chalk_1.default.gray('  status - Check bot status'));
    console.log('\n' + '═'.repeat(60));
    console.log(chalk_1.default.gray('Install optional dependency: npm install telegraf\n'));
}
function printWebhookSetup() {
    console.log(chalk_1.default.bold('\n🔗 Webhook Integration Setup\n'));
    console.log('═'.repeat(60));
    console.log(chalk_1.default.bold('\n📋 Overview\n'));
    console.log('  RuvBot provides webhook endpoints for custom integrations.');
    console.log('  Use webhooks to connect with any messaging platform or service.');
    console.log(chalk_1.default.bold('\n🔌 Available Webhook Endpoints\n'));
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log(`  POST  ${chalk_1.default.cyan('/webhook/message')}        - Receive messages`);
    console.log(`  POST  ${chalk_1.default.cyan('/webhook/event')}          - Receive events`);
    console.log(`  GET   ${chalk_1.default.cyan('/webhook/health')}         - Health check`);
    console.log(`  POST  ${chalk_1.default.cyan('/api/sessions/:id/chat')}  - Chat endpoint`);
    console.log(chalk_1.default.bold('\n📤 Outbound Webhooks\n'));
    console.log('  Configure RuvBot to send responses to your endpoint:');
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log(chalk_1.default.cyan('  export WEBHOOK_URL="https://your-service.com/callback"'));
    console.log(chalk_1.default.cyan('  export WEBHOOK_SECRET="your-shared-secret"'));
    console.log(chalk_1.default.bold('\n📥 Inbound Webhook Format\n'));
    console.log('  Send POST requests with JSON body:');
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log(chalk_1.default.cyan(`  curl -X POST https://your-ruvbot.run.app/webhook/message \\
    -H "Content-Type: application/json" \\
    -H "X-Webhook-Secret: your-secret" \\
    -d '{
      "message": "Hello RuvBot!",
      "userId": "user-123",
      "channelId": "channel-456",
      "metadata": {}
    }'`));
    console.log(chalk_1.default.bold('\n🔐 Security\n'));
    console.log('  1. Always use HTTPS in production');
    console.log('  2. Set a webhook secret for signature verification');
    console.log('  3. Validate the X-Webhook-Signature header');
    console.log('  4. Enable IP allowlisting if possible');
    console.log(chalk_1.default.bold('\n📋 Configuration File\n'));
    console.log(chalk_1.default.gray('  ─────────────────────────────────────'));
    console.log(chalk_1.default.cyan(`  {
    "channels": {
      "webhook": {
        "enabled": true,
        "inbound": {
          "path": "/webhook/message",
          "secret": "\${WEBHOOK_SECRET}"
        },
        "outbound": {
          "url": "\${WEBHOOK_URL}",
          "retries": 3,
          "timeout": 30000
        }
      }
    }
  }`));
    console.log(chalk_1.default.bold('\n🚀 Start with Webhook Support\n'));
    console.log(chalk_1.default.cyan('  ruvbot start --port 3000'));
    console.log(chalk_1.default.gray('  # Webhooks are always available on the API server'));
    console.log('\n' + '═'.repeat(60) + '\n');
}
function createWebhooksCommand() {
    const webhooks = new commander_1.Command('webhooks')
        .alias('wh')
        .description('Configure webhook integrations');
    // List webhooks
    webhooks
        .command('list')
        .description('List configured webhooks')
        .action(() => {
        console.log(chalk_1.default.bold('\n🔗 Configured Webhooks\n'));
        console.log('─'.repeat(50));
        const outboundUrl = process.env.WEBHOOK_URL;
        if (outboundUrl) {
            console.log(chalk_1.default.green('✓ Outbound webhook:'), outboundUrl);
        }
        else {
            console.log(chalk_1.default.gray('○ No outbound webhook configured'));
        }
        console.log();
        console.log('Inbound endpoints (always available):');
        console.log(`  POST ${chalk_1.default.cyan('/webhook/message')}`);
        console.log(`  POST ${chalk_1.default.cyan('/webhook/event')}`);
        console.log(`  POST ${chalk_1.default.cyan('/api/sessions/:id/chat')}`);
        console.log();
    });
    // Test webhook
    webhooks
        .command('test <url>')
        .description('Test a webhook endpoint')
        .option('--payload <json>', 'Custom JSON payload')
        .action(async (url, options) => {
        console.log(chalk_1.default.cyan(`\nTesting webhook: ${url}\n`));
        try {
            const payload = options.payload
                ? JSON.parse(options.payload)
                : { test: true, timestamp: new Date().toISOString() };
            const response = await fetch(url, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload),
            });
            if (response.ok) {
                console.log(chalk_1.default.green('✓ Webhook responded successfully'));
                console.log(`  Status: ${response.status}`);
                const body = await response.text();
                if (body) {
                    console.log(`  Response: ${body.substring(0, 200)}`);
                }
            }
            else {
                console.log(chalk_1.default.red('✗ Webhook failed'));
                console.log(`  Status: ${response.status}`);
            }
        }
        catch (error) {
            console.log(chalk_1.default.red('✗ Failed to reach webhook'));
            console.log(`  Error: ${error instanceof Error ? error.message : 'Unknown'}`);
        }
    });
    return webhooks;
}
exports.default = createChannelsCommand;
//# sourceMappingURL=channels.js.map