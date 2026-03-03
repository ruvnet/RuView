---
name: flow-nexus-challenges
description: Coding challenges, achievements, and leaderboards
---

# Flow Nexus Challenges

Complete coding challenges to earn rUv credits and climb the leaderboard.

## List Challenges
```javascript
mcp__flow-nexus__challenges_list({
  difficulty: "intermediate", // beginner, advanced, expert
  category: "algorithms",
  status: "active",
  limit: 20
})
```

## Get Challenge Details
```javascript
mcp__flow-nexus__challenge_get({
  challenge_id: "two-sum-problem"
})
```

## Submit Solution
```javascript
mcp__flow-nexus__challenge_submit({
  challenge_id: "challenge_id",
  user_id: "your_id",
  solution_code: `
    function solution(nums, target) {
      const map = new Map();
      for (let i = 0; i < nums.length; i++) {
        const complement = target - nums[i];
        if (map.has(complement)) {
          return [map.get(complement), i];
        }
        map.set(nums[i], i);
      }
      return [];
    }
  `,
  language: "javascript",
  execution_time: 45 // milliseconds
})
```

## Complete Challenge
```javascript
mcp__flow-nexus__app_store_complete_challenge({
  challenge_id: "challenge_id",
  user_id: "your_id",
  submission_data: {
    passed_tests: 10,
    total_tests: 10,
    execution_time: 45
  }
})
```

## Leaderboards
```javascript
// Global leaderboard
mcp__flow-nexus__leaderboard_get({
  type: "global", // weekly, monthly, challenge
  limit: 10
})

// Challenge-specific leaderboard
mcp__flow-nexus__leaderboard_get({
  type: "challenge",
  challenge_id: "specific_challenge",
  limit: 25
})
```

## Achievements
```javascript
mcp__flow-nexus__achievements_list({
  user_id: "your_id",
  category: "speed_demon" // Categories vary
})
```

## rUv Credits
```javascript
// Check balance
mcp__flow-nexus__ruv_balance({ user_id: "your_id" })

// View history
mcp__flow-nexus__ruv_history({
  user_id: "your_id",
  limit: 20
})

// Earn credits (automatic on completion)
mcp__flow-nexus__app_store_earn_ruv({
  user_id: "your_id",
  amount: 100,
  reason: "Completed expert challenge",
  source: "challenge"
})
```

## Challenge Categories
- **algorithms**: Classic algorithm problems
- **data-structures**: DS implementation challenges
- **system-design**: Architecture challenges
- **optimization**: Performance challenges
- **security**: Security-focused problems
- **ml-basics**: Machine learning fundamentals

## Tips
1. Start with beginner challenges
2. Review other solutions after completing
3. Optimize for both correctness and speed
4. Complete daily challenges for bonus credits
5. Unlock achievements for extra rewards