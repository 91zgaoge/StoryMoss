import { useState, useCallback, useRef } from 'react';
import toast from 'react-hot-toast';
import { createLogger } from '@/utils/logger';
import type { TextOperation } from '@/types/collab';

const wsLogger = createLogger('websocket:collab');

interface CursorPosition {
  line: number;
  column: number;
}

interface Participant {
  user_id: string;
  user_name: string;
}

interface CollabMessage {
  type: 'join' | 'leave' | 'operation' | 'cursor' | 'ack' | 'sync' | 'error' | 'participants';
  session_id?: string;
  user_id?: string;
  user_name?: string;
  operation?: TextOperation;
  client_version?: number;
  position?: CursorPosition;
  version?: number;
  content?: string;
  message?: string;
  participants?: Participant[];
}

interface UseCollaborationOptions {
  storyId: string;
  chapterId: string;
  userId: string;
  userName: string;
  onRemoteOperation?: (op: TextOperation) => void;
  onUserJoined?: (user: Participant) => void;
  onUserLeft?: (user: Participant) => void;
}

export function useCollaboration({
  storyId,
  chapterId,
  userId,
  userName,
  onRemoteOperation,
  onUserJoined,
  onUserLeft,
}: UseCollaborationOptions) {
  const [isConnected, setIsConnected] = useState(false);
  const [version, setVersion] = useState(0);
  const [participants, setParticipants] = useState<Participant[]>([]);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  const connect = useCallback(() => {
    wsLogger.debug('Connect called', { storyId, chapterId, userId });

    if (!storyId || !chapterId || !userId) {
      wsLogger.debug('Cannot connect: missing params');
      setError('Missing required parameters');
      return;
    }

    // Phase 4: Collaborative editing is reserved for future release
    toast('协同编辑功能即将推出，敬请期待');
    return;

    /* Reserved for future use
    setError(null);
    wsLogger.debug('Connecting to ws://127.0.0.1:8765');

    try {
      const ws = new WebSocket(`ws://127.0.0.1:8765`);
      wsRef.current = ws;

      ws.onopen = () => {
        wsLogger.debug('WebSocket connected');
        setIsConnected(true);
        setError(null);
        
        const joinMsg: CollabMessage = {
          type: 'join',
          session_id: `${storyId}-${chapterId}`,
          user_id: userId,
          user_name: userName,
        };
        wsLogger.debug('Sending join message', { joinMsg });
        ws.send(JSON.stringify(joinMsg));
      };

      ws.onmessage = (event) => {
        wsLogger.debug('Received message', { data: event.data });
        try {
          const msg: CollabMessage = JSON.parse(event.data);

          switch (msg.type) {
            case 'operation':
              if (msg.operation && onRemoteOperation) {
                onRemoteOperation(msg.operation);
              }
              break;
            case 'participants':
              if (msg.participants) {
                setParticipants(msg.participants);
              }
              break;
            case 'ack':
              if (msg.version !== undefined) {
                setVersion(msg.version);
              }
              break;
            case 'error':
              wsLogger.error('Server error', { message: msg.message });
              setError(msg.message || 'Server error');
              break;
          }
        } catch (e) {
          wsLogger.error('Failed to parse message', { error: e });
        }
      };

      ws.onclose = (event) => {
        wsLogger.debug('WebSocket closed', { code: event.code, reason: event.reason });
        setIsConnected(false);
        setParticipants([]);
      };

      ws.onerror = (error) => {
        wsLogger.error('WebSocket error', { error });
        setError('Connection failed');
        toast.error('协同编辑连接失败，请检查网络');
      };
    } catch (e) {
      wsLogger.error('Failed to create WebSocket', { error: e });
      setError('Failed to create connection');
    }
    */
  }, [storyId, chapterId, userId, userName, onRemoteOperation]);

  const disconnect = useCallback(() => {
    wsLogger.debug('Disconnecting...');
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setIsConnected(false);
    setParticipants([]);
    toast('已断开协同编辑连接');
  }, []);

  const sendOperation = useCallback(
    (operation: TextOperation) => {
      wsLogger.debug('Sending operation', { operation });
      if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
        const msg: CollabMessage = {
          type: 'operation',
          operation,
          client_version: version,
        };
        wsRef.current.send(JSON.stringify(msg));
      }
    },
    [version]
  );

  const sendCursorPosition = useCallback((position: CursorPosition) => {
    wsLogger.debug('Sending cursor position', { position });
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      const msg: CollabMessage = {
        type: 'cursor',
        position,
      };
      wsRef.current.send(JSON.stringify(msg));
    }
  }, []);

  return {
    isConnected,
    version,
    participants,
    error,
    connect,
    disconnect,
    sendOperation,
    sendCursorPosition,
  };
}
