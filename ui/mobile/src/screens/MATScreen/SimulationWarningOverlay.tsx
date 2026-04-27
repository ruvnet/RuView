import React from 'react';
import { Modal, Pressable, StyleSheet, Text, View } from 'react-native';

interface Props {
  visible: boolean;
  onAcknowledge: () => void;
}

export const SimulationWarningOverlay: React.FC<Props> = ({ visible, onAcknowledge }) => (
  <Modal visible={visible} transparent animationType="fade">
    <View style={styles.backdrop}>
      <View style={styles.card}>
        <Text style={styles.icon}>&#9888;</Text>
        <Text style={styles.title}>SIMULATED DATA</Text>
        <Text style={styles.body}>
          NOT CONNECTED TO REAL SENSORS{'\n\n'}
          All survivor detections, vital signs, and alerts displayed on this screen are
          generated from simulated data and do not reflect actual conditions.
        </Text>
        <Pressable style={styles.button} onPress={onAcknowledge}>
          <Text style={styles.buttonText}>I UNDERSTAND</Text>
        </Pressable>
      </View>
    </View>
  </Modal>
);

const styles = StyleSheet.create({
  backdrop: {
    flex: 1,
    backgroundColor: 'rgba(0,0,0,0.85)',
    justifyContent: 'center',
    alignItems: 'center',
    padding: 24,
  },
  card: {
    backgroundColor: '#1a1a2e',
    borderRadius: 16,
    padding: 32,
    alignItems: 'center',
    borderWidth: 2,
    borderColor: '#e74c3c',
    maxWidth: 420,
    width: '100%',
  },
  icon: {
    fontSize: 48,
    color: '#e74c3c',
    marginBottom: 12,
  },
  title: {
    fontSize: 22,
    fontWeight: '800',
    color: '#e74c3c',
    textAlign: 'center',
    marginBottom: 16,
    letterSpacing: 1,
  },
  body: {
    fontSize: 15,
    color: '#cccccc',
    textAlign: 'center',
    lineHeight: 22,
    marginBottom: 28,
  },
  button: {
    backgroundColor: '#e74c3c',
    paddingHorizontal: 36,
    paddingVertical: 14,
    borderRadius: 8,
  },
  buttonText: {
    color: '#ffffff',
    fontWeight: '700',
    fontSize: 16,
    letterSpacing: 0.5,
  },
});
