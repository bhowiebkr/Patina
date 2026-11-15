// A node of the graph: a header in the family color and a row for each pin (docs/gui-visual-guide.md).

import { memo } from 'react';
import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import type { GraphNode, Pin } from './types';
import { FAMILIES } from './theme';

export type PatinaNodeType = Node<{ node: GraphNode; pitch: number }, 'patina'>;

function label(name: string, pin: Pin): string {
  return pin.width > 1 ? `${name}[${pin.width - 1}:0]` : name;
}

function handleStyle(pin: Pin, pitch: number) {
  return { top: pin.y, height: pin.width > 1 ? pin.width * pitch + 4 : undefined };
}

function NodeViewImpl({ data, selected }: NodeProps<PatinaNodeType>) {
  const { node, pitch } = data;
  return (
    <div className={selected ? 'patina-node selected' : 'patina-node'} style={{ width: node.w, height: node.h }}>
      <div className="patina-node-header" style={{ background: FAMILIES[node.family % FAMILIES.length] }}>
        {node.title}
      </div>
      {node.inputs.map((pin, k) => (
        <div key={`i${k}`} className="patina-pin in" style={{ top: pin.y }}>
          {label(`in${k}`, pin)}
        </div>
      ))}
      {node.outputs.map((pin, k) => (
        <div key={`o${k}`} className="patina-pin out" style={{ top: pin.y }}>
          {label(pin.width > 1 ? 'bus' : 'out', pin)}
        </div>
      ))}
      {node.inputs.map((pin, k) => (
        <Handle key={`hi${k}`} id={`i${k}`} type="target" position={Position.Left} isConnectable={false} style={handleStyle(pin, pitch)} />
      ))}
      {node.outputs.map((pin, k) => (
        <Handle key={`ho${k}`} id={`o${k}`} type="source" position={Position.Right} isConnectable={false} style={handleStyle(pin, pitch)} />
      ))}
    </div>
  );
}

export const NodeView = memo(NodeViewImpl);
