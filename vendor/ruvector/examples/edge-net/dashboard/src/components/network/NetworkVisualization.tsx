import { useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { useNetworkStore } from '../../stores/networkStore';

interface Node {
  x: number;
  y: number;
  vx: number;
  vy: number;
  connections: number[];
}

export function NetworkVisualization() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const nodesRef = useRef<Node[]>([]);
  const animationRef = useRef<number | undefined>(undefined);
  const { stats } = useNetworkStore();

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const resizeCanvas = () => {
      canvas.width = canvas.offsetWidth * window.devicePixelRatio;
      canvas.height = canvas.offsetHeight * window.devicePixelRatio;
      ctx.scale(window.devicePixelRatio, window.devicePixelRatio);
    };

    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);

    // Initialize nodes
    const nodeCount = 30;
    nodesRef.current = Array.from({ length: nodeCount }, (_, i) => ({
      x: Math.random() * canvas.offsetWidth,
      y: Math.random() * canvas.offsetHeight,
      vx: (Math.random() - 0.5) * 0.5,
      vy: (Math.random() - 0.5) * 0.5,
      connections: Array.from(
        { length: Math.floor(Math.random() * 3) + 1 },
        () => Math.floor(Math.random() * nodeCount)
      ).filter((c) => c !== i),
    }));

    const animate = () => {
      const width = canvas.offsetWidth;
      const height = canvas.offsetHeight;

      ctx.clearRect(0, 0, width, height);

      // Update and draw nodes
      nodesRef.current.forEach((node) => {
        // Update position
        node.x += node.vx;
        node.y += node.vy;

        // Bounce off edges
        if (node.x < 0 || node.x > width) node.vx *= -1;
        if (node.y < 0 || node.y > height) node.vy *= -1;

        // Draw connections
        node.connections.forEach((targetIdx) => {
          const target = nodesRef.current[targetIdx];
          if (target) {
            const distance = Math.hypot(target.x - node.x, target.y - node.y);
            const maxDistance = 150;

            if (distance < maxDistance) {
              const opacity = 1 - distance / maxDistance;
              ctx.beginPath();
              ctx.moveTo(node.x, node.y);
              ctx.lineTo(target.x, target.y);
              ctx.strokeStyle = `rgba(14, 165, 233, ${opacity * 0.3})`;
              ctx.lineWidth = 1;
              ctx.stroke();
            }
          }
        });
      });

      // Draw nodes
      nodesRef.current.forEach((node, i) => {
        const isActive = i < Math.floor(nodeCount * (stats.activeNodes / stats.totalNodes));

        // Glow
        const gradient = ctx.createRadialGradient(node.x, node.y, 0, node.x, node.y, 15);
        gradient.addColorStop(0, isActive ? 'rgba(14, 165, 233, 0.3)' : 'rgba(100, 100, 100, 0.1)');
        gradient.addColorStop(1, 'transparent');
        ctx.fillStyle = gradient;
        ctx.fillRect(node.x - 15, node.y - 15, 30, 30);

        // Node
        ctx.beginPath();
        ctx.arc(node.x, node.y, 4, 0, Math.PI * 2);
        ctx.fillStyle = isActive ? '#0ea5e9' : '#52525b';
        ctx.fill();
      });

      animationRef.current = requestAnimationFrame(animate);
    };

    animate();

    return () => {
      window.removeEventListener('resize', resizeCanvas);
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, [stats.activeNodes, stats.totalNodes]);

  return (
    <motion.div
      className="crystal-card p-4 h-[300px]"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
    >
      <h3 className="text-sm font-medium text-zinc-400 mb-2">Network Topology</h3>
      <canvas
        ref={canvasRef}
        className="w-full h-full rounded-lg"
        style={{ background: 'rgba(0, 0, 0, 0.3)' }}
      />
    </motion.div>
  );
}
