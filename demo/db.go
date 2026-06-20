package main

func (s *Store) create(value string) Item {
	s.mu.Lock()
	defer s.mu.Unlock()

	item := Item{ID: s.nextID, Value: value}
	s.items[s.nextID] = item
	s.nextID++
	return item
}

func (s *Store) get(id int) (Item, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	item, ok := s.items[id]
	return item, ok
}

func (s *Store) listIDs() []int {
	s.mu.Lock()
	defer s.mu.Unlock()

	ids := make([]int, 0, len(s.items))
	for id := range s.items {
		ids = append(ids, id)
	}
	return ids
}
