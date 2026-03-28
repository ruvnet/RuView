import React from 'react';
import { Text } from 'react-native';
import { render, screen } from '@testing-library/react-native';

jest.mock('@/services/ws.service', () => ({
  wsService: {
    subscribe: jest.fn(() => jest.fn()),
    connect: jest.fn(),
    disconnect: jest.fn(),
    getStatus: jest.fn(() => 'disconnected'),
  },
}));

import { usePoseStream } from '@/hooks/usePoseStream';
import { usePoseStore } from '@/stores/poseStore';

const HookConsumer = () => {
  const { connectionStatus, lastFrame, isSimulated } = usePoseStream();

  return React.createElement(
    React.Fragment,
    null,
    React.createElement(Text, { testID: 'status' }, connectionStatus),
    React.createElement(Text, { testID: 'simulated' }, String(isSimulated)),
    React.createElement(Text, { testID: 'frame' }, lastFrame ? 'present' : 'none'),
  );
};

describe('usePoseStream', () => {
  beforeEach(() => {
    usePoseStore.getState().reset();
    const { wsService } = require('@/services/ws.service');
    wsService.subscribe.mockClear();
    wsService.connect.mockClear();
  });

  it('returns the expected store-backed values', () => {
    render(React.createElement(HookConsumer));

    expect(screen.getByTestId('status').props.children).toBe('disconnected');
    expect(screen.getByTestId('simulated').props.children).toBe('false');
    expect(screen.getByTestId('frame').props.children).toBe('none');
  });

  it('does not subscribe or reconnect on render', () => {
    const { wsService } = require('@/services/ws.service');

    render(React.createElement(HookConsumer));

    expect(wsService.subscribe).not.toHaveBeenCalled();
    expect(wsService.connect).not.toHaveBeenCalled();
  });
});
