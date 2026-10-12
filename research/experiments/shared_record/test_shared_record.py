"""Problem-family invariants retained from the original study."""
import json
import random
import unittest
import problems


class Problems(unittest.TestCase):
    def test_every_family_is_consistent(self):
        for name, problem in problems.FAMILIES.items():
            with self.subTest(family=name):
                a = problem.generate(random.Random(7))
                b = problem.generate(random.Random(7))
                self.assertEqual(a, b, 'generation must be deterministic in the seed')
                reference = problem.reference(a)
                problem.validate(a, reference)
                fallback = problem.fallback(a)
                problem.validate(a, fallback)
                self.assertLessEqual(problem.cost(a, reference), problem.cost(a, fallback))
                self.assertGreater(problem.cost(a, fallback), 0)
                with self.assertRaises((ValueError, TypeError, IndexError, KeyError)):
                    problem.validate(a, 'not a solution')
                json.dumps(a), json.dumps(reference)
                self.assertTrue(problem.shape.strip(), 'every family states its generator for the prompt')

    def test_qap_reference_delta_matches_full_recomputation(self):
        qap = problems.FAMILIES['qap']
        for seed in range(4):
            rng = random.Random(seed)
            n = rng.randint(6, 14)

            def symmetric():
                m = [[0] * n for _ in range(n)]
                for i in range(n):
                    for j in range(i + 1, n):
                        m[i][j] = m[j][i] = rng.randint(0, 20)
                return m
            inst = {'flow': symmetric(), 'distance': symmetric()}
            p = list(range(n))
            current = qap.cost(inst, p)
            improved = True
            while improved:
                improved = False
                for i in range(n):
                    for j in range(i + 1, n):
                        p[i], p[j] = p[j], p[i]
                        trial = qap.cost(inst, p)
                        if trial < current:
                            current, improved = trial, True
                        else:
                            p[i], p[j] = p[j], p[i]
            self.assertEqual(qap.reference(inst), p)

    def test_validators_reject_specific_violations(self):
        tsp = problems.FAMILIES['tsp']
        inst = tsp.generate(random.Random(1))
        with self.assertRaises(ValueError):
            tsp.validate(inst, [0] * len(inst['points']))
        col = problems.FAMILIES['coloring']
        inst = col.generate(random.Random(1))
        with self.assertRaises(ValueError):
            col.validate(inst, [0] * inst['n'])
        mkp = problems.FAMILIES['mkp']
        inst = mkp.generate(random.Random(1))
        with self.assertRaises(ValueError):
            mkp.validate(inst, list(range(len(inst['values']))))
        cover = problems.FAMILIES['setcover']
        inst = cover.generate(random.Random(1))
        with self.assertRaises(ValueError):
            cover.validate(inst, [])

    def test_instance_splits_differ_and_are_frozen(self):
        public = problems.instances('seed', 'tsp', 'public', pack='p0', count=5)
        hidden = problems.instances('seed', 'tsp', 'hidden', pack='p0', count=10)
        self.assertEqual(len(public), 5)
        self.assertEqual(len(hidden), 10)
        self.assertNotIn(public[0], hidden)
        self.assertEqual(public, problems.instances('seed', 'tsp', 'public', pack='p0', count=5))
        self.assertNotEqual(public, problems.instances('other', 'tsp', 'public', pack='p0', count=5))


if __name__ == "__main__":
    unittest.main()
