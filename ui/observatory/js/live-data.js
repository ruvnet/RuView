/** Live presence alone is not evidence of a measured pose. Demo figures remain illustrative. */
export function visiblePersons(data, live, modality) {
  const persons = Array.isArray(data?.persons) ? data.persons : [];
  if (!live) return persons;
  // RSSI-only streams carry no pose: never render skeletons from them.
  if (modality === 'rssi') return [];
  return persons.filter(person => Array.isArray(person?.keypoints) &&
    person.keypoints.some(kp => Number.isFinite(kp?.confidence) && kp.confidence > 0));
}
