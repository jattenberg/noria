use ops::grouped::GroupedOperation;
use ops::grouped::GroupedOperator;
use randomkit::dist::Laplace;
use randomkit::{Rng, Sample};
use std::cmp::Ordering;
use std::f64;
use std::collections::HashMap;
use std::fmt;

use prelude::*;

// Define the Binary, Logarithmic, and Hybrid Mechanisms

// Binary Mechanism (bounded in a window of size T)
#[derive(Serialize, Deserialize)]
pub struct BinaryMechanism {
    #[serde(skip)]
    alphas: Option<HashMap<u32, u32>>,
    #[serde(skip)]
    noisy_alphas: Option<HashMap<u32, f64>>,
    T: f64,
    t: f64,
    eps: f64,
    #[serde(skip)]
    noise_distr: Option<Laplace>,
    #[serde(skip)]
    rng: Option<Rng>,
    prev_output: f64,
}

impl Clone for BinaryMechanism {
    fn clone(&self) -> Self {
        assert!(self.noise_distr.is_none());
        assert!(self.rng.is_none());
        assert!(self.alphas.is_none());
        assert!(self.noisy_alphas.is_none());
        BinaryMechanism {
            t: self.t,
            T: self.T,
            prev_output: self.prev_output,
            eps: self.eps,
            noise_distr: None,
            rng: None,
            alphas: None,
            noisy_alphas: None,
        }
    }
}

impl BinaryMechanism {
    pub fn new(T: f64, e: f64) -> BinaryMechanism {
        BinaryMechanism {
            alphas: None,
            noisy_alphas: None,
            T: T,
            t: 1.0,
            eps: e,
            noise_distr: None,
            rng: None,
            prev_output: 0.0,
        }
    }

    pub fn set_noise_distr(&mut self) {
        self.noise_distr = Some(Laplace::new(0.0, self.T.log2()/self.eps).unwrap());
        self.rng = Some(Rng::from_seed(1));
    }

    pub fn initialize_psums(&mut self) {
        self.alphas = Some(HashMap::new());
        self.noisy_alphas = Some(HashMap::new());
    }
    
    pub fn step_forward(&mut self, element: i64) -> f64 {
        if self.t > self.T {
            return self.prev_output;
        }

        // Get lowest nonzero bit
        let t_prime = self.t as i32;
        let i = ((t_prime & -t_prime) as f64).log2() as u32;
        
        // Create and store a new psum that includes this timestep
        let mut value = element as u32;
        for j in 0..i {
            value += *self.alphas.as_mut().unwrap().entry(j).or_insert(1000); // TODO: better default value to indicate error
            self.alphas.as_mut().unwrap().insert(
                i,
                value,
            );
        }

        // Delete any psums contained in the new psum     
        for j in 0..i {
            self.alphas.as_mut().unwrap().remove(&j);
            self.noisy_alphas.as_mut().unwrap().remove(&j);
        }

        // Update noisy_alphas
        let noise = self.noise_distr.unwrap().sample(self.rng.as_mut().unwrap());    
        self.noisy_alphas.as_mut().unwrap().insert(
            i,
            (value as f64) + noise,
        );

        // Calculate the output
        let t_bin = format!("{:b}", self.t as u32).chars().rev().collect::<String>();      
        let mut output = 0.0;        
        for char_index in t_bin.char_indices() {
            let (j, elt) = char_index;
            if elt == '1' {
                output += *self.noisy_alphas.as_mut().unwrap().entry(j as u32).or_insert(1000.0);
            }
        }
        // Update previous_output, increment t and t_bin, and return                           
        self.t += 1.0;
        self.prev_output = output;
        output
    }
}

// Logarithmic mechanism (unbounded)
#[derive(Serialize, Deserialize)]
pub struct LogarithmicMechanism {
    beta: f64,
    t: f64,
    prev_output: f64,
    eps: f64,
    #[serde(skip)]
    noise_distr: Option<Laplace>,
    #[serde(skip)]
    rng: Option<Rng>,
}

impl Clone for LogarithmicMechanism {
    fn clone(&self) -> Self {
        assert!(self.noise_distr.is_none());
        assert!(self.rng.is_none());
        LogarithmicMechanism {
            beta: self.beta,
            t: self.t,
            prev_output: self.prev_output,
            eps: self.eps,
            noise_distr: None,
            rng: None,
        }
    }
}

impl LogarithmicMechanism {
    pub fn new(e: f64) -> LogarithmicMechanism {
        LogarithmicMechanism {
            beta: 0.0,
            t: 1.0,
            prev_output: 0.0,
            eps: e,
            noise_distr: None,
            rng: None,
        }
    }

    pub fn set_noise_distr(&mut self) -> () {
        self.noise_distr = Some(Laplace::new(0.0, 1.0/self.eps).unwrap());
        self.rng = Some(Rng::from_seed(1));
    }

    pub fn step_forward(&mut self, element: i64) -> f64 {
        self.beta += (element as u32) as f64;
        // If t is not a power of 2, return previous output
        if self.t.log2().floor() != self.t.log2().ceil() {
            self.t += 1.0;
            return self.prev_output
        }
        // t is a power of 2; update beta and return new output
        let noise = self.noise_distr.unwrap().sample(self.rng.as_mut().unwrap());
        self.beta += noise;
        self.prev_output = self.beta;
        self.t += 1.0;
        self.beta
    }
}

// Hybrid Mechanism (unbounded): composition of Logarithmic & Binary mechanisms
#[derive(Clone, Serialize, Deserialize)]
pub struct HybridMechanism {
    l: LogarithmicMechanism,
    b: BinaryMechanism,
    e: f64,
    t: f64,
}

impl fmt::Debug for HybridMechanism {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HybridMechanism {{ epsilon: {}, t: {}, current T: {} }}", self.e, self.t, self.b.T)
    }
}

impl HybridMechanism {
    pub fn new(e: f64) -> HybridMechanism {
        HybridMechanism {
            l: LogarithmicMechanism::new(e/2.0),
            b: BinaryMechanism::new(2.0, e/2.0),
            e: e,
            t: 1.0,
        }
    }

    pub fn step_forward(&mut self, element: i64) -> f64 {
        // Always step Log Mech forward; will only do an update if power of 2.
        let l_out = self.l.step_forward(element);

        // If t is a power of 2, initialize new binary mechanism.
        if self.t > 1.0 && self.t.log2().floor() == self.t.log2().ceil() {
            self.b = BinaryMechanism::new(self.t, self.e/2.0);
            self.t += 1.0;
            return l_out
        }

        // t is not a power of 2; update binary mechanism.
        if self.t > 1.0 {
            let b_out = self.b.step_forward(element);
            self.t += 1.0;
            return l_out + b_out
        }
        // t <= 1.0
        self.t += 1.0;
        l_out
    }
}

/// Supported aggregation operators.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DpAggregation {
    /// Count the number of records for each group. The value for the `over` column is ignored.
    COUNT,
}

impl DpAggregation {
    /// Construct a new `Aggregator` that performs this operation.
    ///
    /// The aggregation will aggregate the value in column number `over` from its inputs (i.e.,
    /// from the `src` node in the graph), and use the columns in the `group_by` array as a group
    /// identifier. The `over` column should not be in the `group_by` array.
    pub fn over(
        self,
        src: NodeIndex,
        over: usize,
        group_by: &[usize],
        eps: f64,
    ) -> GroupedOperator<DpAggregator> {
        assert!(
            !group_by.iter().any(|&i| i == over),
            "cannot group by aggregation column"
        );
        GroupedOperator::new(
            src,
            DpAggregator {
                op: self,
                over: over,
                group: group_by.into(),
                counter: HybridMechanism::new(eps),
            },
        )
    }
}

/// Aggregator implements a Soup node that performs common aggregation operations such as counts
/// and sums.
///
/// `Aggregator` nodes are constructed through `Aggregation` variants using `Aggregation::new`.
///
/// When a new record arrives, the aggregator will first query the currently aggregated value for
/// the new record's group by doing a query into its own output. The aggregated column
/// (`self.over`) of the incoming record is then added to the current aggregation value according
/// to the operator in use (`COUNT` always adds/subtracts 1, `SUM` adds/subtracts the value of the
/// value in the incoming record. The output record is constructed by concatenating the columns
/// identifying the group, and appending the aggregated value. For example, for a sum with
/// `self.over == 1`, a previous sum of `3`, and an incoming record with `[a, 1, x]`, the output
/// would be `[a, x, 4]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpAggregator {
    op: DpAggregation,
    over: usize,
    group: Vec<usize>,
    counter: HybridMechanism,
}

impl GroupedOperation for DpAggregator {
    type Diff = i64;

    // Called at the beginning of on_connect()
    fn setup(&mut self, parent: &Node) {
        assert!(
            self.over < parent.fields().len(),
            "cannot aggregate over non-existing column"
        );
        // Initialize Option<...> fields in counter.
        self.counter.l.set_noise_distr();
        self.counter.b.set_noise_distr();
        self.counter.b.initialize_psums();
    }

    fn group_by(&self) -> &[usize] {
        &self.group[..]
    }

    fn to_diff(&self, _r: &[DataType], pos: bool) -> Self::Diff {
        match self.op {
            DpAggregation::COUNT if pos => 1,
            DpAggregation::COUNT => -1,
        }
    }

    fn apply(
        &mut self,
        _current: Option<&DataType>,
        diffs: &mut Iterator<Item = Self::Diff>,
    ) -> DataType {
        // "current" is superfluous, already tracked in counter state.
        // LATER: for increment and decrement counters
        // TODO: should both pos and neg take the 0's as well? How is clocking affected by the split?
        // Should -1's be treated as zeros in pos counter and vice versa (if so, below code won't work)?
        // pos = diffs.into_iter().filter(|d| d > 0).map(|d| self.pos_counter.step_forward(d)).last().into()
        // neg = diffs.into_iter().filter(|d| d < 0).map(|d| self.neg_counter.step_forward(-1*d)).last().into()
        // pos - neg
        diffs.into_iter().map(|d| self.counter.step_forward(d as i64)).last().unwrap().into()
    }

    fn description(&self) -> String {
        let op_string : String = match self.op {
            DpAggregation::COUNT => "|*|".into(),
        };
        let group_cols = self
            .group
            .iter()
            .map(|g| g.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("{} γ[{}]", op_string, group_cols)
    }

    // Temporary: for now, disable backwards queries
    fn requires_full_materialization(&self) -> bool {
        true
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use ops;

    fn setup(mat: bool) -> ops::test::MockGraph {
        let mut g = ops::test::MockGraph::new();
        let s = g.add_base("source", &["x", "y"]);
        g.set_op(
            "identity",
            &["x", "ys"],
            DpAggregation::COUNT.over(s.as_global(), 1, &[0], 0.1), // epsilon = 0.1
            mat,
        );
        g
    }

    fn setup_multicolumn(mat: bool) -> ops::test::MockGraph {
        let mut g = ops::test::MockGraph::new();
        let s = g.add_base("source", &["x", "y", "z"]);
        g.set_op(
            "identity",
            &["x", "z", "ys"],
            DpAggregation::COUNT.over(s.as_global(), 1, &[0, 2], 0.1), // epsilon = 0.1
            mat,
        );
        g
    }

    #[test]
    fn it_describes() {
        let s = 0.into();

        let c = DpAggregation::COUNT.over(s, 1, &[0, 2], 0.1); // epsilon = 0.1
        assert_eq!(c.description(), "|*| γ[0, 2]");
    }

    #[test]
    fn it_forwards() {
        let mut c = setup(true);

        let u: Record = vec![1.into(), 1.into()].into();

        // first row for a group should emit +1 for that group
        let rs = c.narrow_one(u, true);
        assert_eq!(rs.len(), 1);
        let mut rs = rs.into_iter();

        match rs.next().unwrap() {
            Record::Positive(r) => {
                assert_eq!(r[0], 1.into());
                // Should be within 50 of true count w/ Pr >= 99.3%
                println!("r[1]: {}", r[1]);
                assert!(r[1] <= DataType::from(51.0));
                assert!(r[1] >= DataType::from(-49.0));
            }
            _ => unreachable!(),
        }

        let u: Record = vec![2.into(), 2.into()].into();

        // first row for a second group should emit +1 for that new group
        let rs = c.narrow_one(u, true);
        assert_eq!(rs.len(), 1);
        let mut rs = rs.into_iter();

        match rs.next().unwrap() {
            Record::Positive(r) => {
                assert_eq!(r[0], 2.into());
                // Should be within 50 of true count w/ Pr >= 99.3%
                assert!(r[1] <= DataType::from(51.0));
                assert!(r[1] >= DataType::from(-49.0));
            }
            _ => unreachable!(),
        }

        let u: Record = vec![1.into(), 2.into()].into();

        // second row for a group should emit -1 and +2
        let rs = c.narrow_one(u, true);
        assert_eq!(rs.len(), 2); // Why is rs.len = 2 for this record?
        let mut rs = rs.into_iter();

        match rs.next().unwrap() {
            Record::Negative(r) => {
                assert_eq!(r[0], 1.into());
                assert_eq!(r[1], 1.into());
            }
            _ => unreachable!(),
        }
        match rs.next().unwrap() {
            Record::Positive(r) => {
                assert_eq!(r[0], 1.into());
                assert_eq!(r[1], 2.into());
            }
            _ => unreachable!(),
        }

        let u = (vec![1.into(), 1.into()], false); // false indicates a negative record

        // negative row for a group should emit -2 and +1
        let rs = c.narrow_one_row(u, true);
        assert_eq!(rs.len(), 2);
        let mut rs = rs.into_iter();

        match rs.next().unwrap() {
            Record::Negative(r) => {
                assert_eq!(r[0], 1.into());
                assert_eq!(r[1], 2.into());
            }
            _ => unreachable!(),
        }
        match rs.next().unwrap() {
            Record::Positive(r) => {
                assert_eq!(r[0], 1.into());
                assert_eq!(r[1], 1.into());
            }
            _ => unreachable!(),
        }

        let u = vec![
            (vec![1.into(), 1.into()], false),
            (vec![1.into(), 1.into()], true),
            (vec![1.into(), 2.into()], true),
            (vec![2.into(), 2.into()], false),
            (vec![2.into(), 2.into()], true),
            (vec![2.into(), 3.into()], true),
            (vec![2.into(), 1.into()], true),
            (vec![3.into(), 3.into()], true),
        ];

        // multiple positives and negatives should update aggregation value by appropriate amount
        let rs = c.narrow_one(u, true);
        assert_eq!(rs.len(), 5); // one - and one + for each group, except 3 which is new
                                 // group 1 lost 1 and gained 2
        assert!(rs.iter().any(|r| if let Record::Negative(ref r) = *r {
            r[0] == 1.into() && r[1] == 1.into()
        } else {
            false
        }));
        assert!(rs.iter().any(|r| if let Record::Positive(ref r) = *r {
            r[0] == 1.into() && r[1] == 2.into()
        } else {
            false
        }));
        // group 2 lost 1 and gained 3
        assert!(rs.iter().any(|r| if let Record::Negative(ref r) = *r {
            r[0] == 2.into() && r[1] == 1.into()
        } else {
            false
        }));
        assert!(rs.iter().any(|r| if let Record::Positive(ref r) = *r {
            r[0] == 2.into() && r[1] == 3.into()
        } else {
            false
        }));
        // group 3 lost 0 and gained 1
        assert!(rs.iter().any(|r| if let Record::Positive(ref r) = *r {
            r[0] == 3.into() && r[1] == 1.into()
        } else {
            false
        }));
    }
}
