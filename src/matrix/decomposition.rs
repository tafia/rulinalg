//! Matrix Decompositions
//!
//! References:
//! 1. [On Matrix Balancing and EigenVector computation]
//! (http://arxiv.org/pdf/1401.5766v1.pdf), James, Langou and Lowery
//!
//! 2. [The QR algorithm for eigen decomposition]
//! (http://people.inf.ethz.ch/arbenz/ewp/Lnotes/chapter4.pdf)
//!
//! 3. [Computation of the SVD]
//! (http://www.cs.utexas.edu/users/inderjit/public_papers/HLA_SVD.pdf)

use std::any::Any;
use std::cmp;
use std::ops::{Mul, Add, Div, Sub, Neg};
use std::slice;

use matrix::{Matrix, MatrixSlice, MatrixSliceMut};
use matrix::{back_substitution, forward_substitution, parity};
use matrix::slice::{BaseSlice, BaseSliceMut};
use vector::Vector;
use Metric;
use utils;
use error::{Error, ErrorKind};

use libnum::{One, Zero, Float, Signed};
use libnum::{cast, abs};

/// Trait implementing matrix decompositions
pub trait Decomposition<T>: BaseSlice<T> {

    /// Solves the equation `Ax = y`.
    ///
    /// Requires a Vector `y` as input.
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::vector::Vector;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let a = Matrix::new(2,2, vec![2.0,3.0,1.0,2.0]);
    /// let y = Vector::new(vec![13.0,8.0]);
    ///
    /// let x = a.solve(y).unwrap();
    ///
    /// assert_eq!(*x.data(), vec![2.0, 3.0]);
    /// ```
    ///
    /// # Panics
    ///
    /// - The matrix column count and vector size are different.
    /// - The matrix is not square.
    ///
    /// # Failures
    ///
    /// - The matrix cannot be decomposed into an LUP form to solve.
    /// - There is no valid solution as the matrix is singular.
    fn solve(&self, y: Vector<T>) -> Result<Vector<T>, Error>
        where T: Any + Float,
              for <'a> &'a Matrix<T>: Mul<&'a Self, Output=Matrix<T>>,
    {
        let (l, u, p) = try!(self.lup_decomp());

        let b = try!(forward_substitution(&l, p * y));
        back_substitution(&u, b)
    }

    /// Computes the inverse of the matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let a = Matrix::new(2,2, vec![2.,3.,1.,2.]);
    /// let inv = a.inverse().expect("This matrix should have an inverse!");
    ///
    /// let I = a * inv;
    ///
    /// assert_eq!(*I.data(), vec![1.0,0.0,0.0,1.0]);
    /// ```
    ///
    /// # Panics
    ///
    /// - The matrix is not square.
    ///
    /// # Failures
    ///
    /// - The matrix could not be LUP decomposed.
    /// - The matrix has zero determinant.
    fn inverse(&self) -> Result<Matrix<T>, Error>
        where T: Any + Float,
              for <'a> &'a Matrix<T>: Mul<&'a Self, Output=Matrix<T>>,
              for <'a> &'a Matrix<T>: Mul<Vector<T>, Output=Vector<T>>
    {
        assert!(self.rows() == self.cols(), "Matrix is not square.");

        let mut inv_t_data = Vec::<T>::new();
        let (l, u, p) = try!(self.lup_decomp().map_err(|_| {
            Error::new(ErrorKind::DecompFailure,
                       "Could not compute LUP factorization for inverse.")
        }));

        let mut d = T::one();

        unsafe {
            for i in 0..l.cols {
                d = d * *l.get_unchecked([i, i]);
                d = d * *u.get_unchecked([i, i]);
            }
        }

        if d == T::zero() {
            return Err(Error::new(ErrorKind::DecompFailure,
                                  "Matrix is singular and cannot be inverted."));
        }

        for i in 0..self.rows() {
            let mut id_col = vec![T::zero(); self.cols()];
            id_col[i] = T::one();

            let b = forward_substitution(&l, &p * Vector::new(id_col))
                .expect("Matrix is singular AND has non-zero determinant!?");
            inv_t_data.append(&mut back_substitution(&u, b)
                .expect("Matrix is singular AND has non-zero determinant!?")
                .into_vec());

        }

        Ok(Matrix::new(self.rows(), self.cols(), inv_t_data).transpose())
    }

    /// Computes the determinant of the matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let a = Matrix::new(3,3, vec![1.0,2.0,0.0,
    ///                               0.0,3.0,4.0,
    ///                               5.0, 1.0, 2.0]);
    ///
    /// let det = a.det();
    ///
    /// ```
    ///
    /// # Panics
    ///
    /// - The matrix is not square.
    fn det(&self) -> T
        where T: Any + Float,
              for <'a> &'a Matrix<T>: Mul<&'a Self, Output=Matrix<T>>,
    {
        assert!(self.rows() == self.cols(), "Matrix is not square.");

        let n = self.cols();

        if self.is_diag() {
            let mut d = T::one();

            unsafe {
                for i in 0..n {
                    d = d * *self.get_unchecked([i, i]);
                }
            }

            return d;
        }

        if n == 2 {
            unsafe {
                (*self.get_unchecked([0, 0]) * *self.get_unchecked([1, 1])) - 
                    (*self.get_unchecked([0, 1]) * *self.get_unchecked([1, 0]))
            }
        } else if n == 3 {
            unsafe {
                (*self.get_unchecked([0, 0]) * *self.get_unchecked([1, 1]) * *self.get_unchecked([2, 2])) +
                (*self.get_unchecked([0, 1]) * *self.get_unchecked([1, 2]) * *self.get_unchecked([2, 0])) +
                (*self.get_unchecked([0, 2]) * *self.get_unchecked([1, 0]) * *self.get_unchecked([2, 1])) -
                (*self.get_unchecked([0, 0]) * *self.get_unchecked([1, 2]) * *self.get_unchecked([2, 1])) -
                (*self.get_unchecked([0, 1]) * *self.get_unchecked([1, 0]) * *self.get_unchecked([2, 2])) -
                (*self.get_unchecked([0, 2]) * *self.get_unchecked([1, 1]) * *self.get_unchecked([2, 0]))
            }
        } else {
            let (l, u, p) = self.lup_decomp().expect("Could not compute LUP decomposition.");

            let mut d = T::one();

            unsafe {
                for i in 0..l.cols {
                    d = d * *l.get_unchecked([i, i]);
                    d = d * *u.get_unchecked([i, i]);
                }
            }

            let sgn = parity(&p);

            sgn * d
        }
    }

    /// Cholesky decomposition
    ///
    /// Returns the cholesky decomposition of a positive definite matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let m = Matrix::new(3,3, vec![1.0,0.5,0.5,0.5,1.0,0.5,0.5,0.5,1.0]);
    ///
    /// let l = m.cholesky();
    /// ```
    ///
    /// # Panics
    ///
    /// - The matrix is not square.
    ///
    /// # Failures
    ///
    /// - Matrix is not positive definite.
    fn cholesky(&self) -> Result<Matrix<T>, Error>
        where T: Any + Float,
    {
        assert!(self.rows() == self.cols(),
                "Matrix must be square for Cholesky decomposition.");

        let mut new_data = Vec::<T>::with_capacity(self.rows() * self.cols());

        for (i, row) in self.iter_rows().enumerate() {

            for (j, data) in row.iter().enumerate() {

                if j > i {
                    new_data.push(T::zero());
                    continue;
                }

                let mut sum = T::zero();
                for k in 0..j {
                    sum = sum + (new_data[i * self.cols() + k] * new_data[j * self.cols() + k]);
                }

                if j == i {
                    new_data.push((*data - sum).sqrt());
                } else {
                    let p = (*data - sum) / new_data[j * self.cols() + j];

                    if !p.is_finite() {
                        return Err(Error::new(ErrorKind::DecompFailure,
                                              "Matrix is not positive definite."));
                    } else {

                    }
                    new_data.push(p);
                }
            }
        }

        Ok(Matrix {
            rows: self.rows(),
            cols: self.cols(),
            data: new_data,
        })
    }

    /// Compute the QR decomposition of the matrix.
    ///
    /// Returns the tuple (Q,R).
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let m = Matrix::new(3,3, vec![1.0,0.5,0.5,0.5,1.0,0.5,0.5,0.5,1.0]);
    ///
    /// let (q, r) = m.qr_decomp().unwrap();
    /// ```
    ///
    /// # Failures
    ///
    /// - Cannot compute the QR decomposition.
    fn qr_decomp(self) -> Result<(Matrix<T>, Matrix<T>), Error>
        where T: Any + Float,
    {
        let m = self.rows();
        let n = self.cols();

        let mut q = Matrix::<T>::identity(m);
        let mut r = self.into_matrix(); // no-op if `Matrix`

        for i in 0..(n - ((m == n) as usize)) {
            let holder_transform: Result<Matrix<T>, Error>;
            {
                let lower_slice = MatrixSlice::from_matrix(&r, [i, i], m - i, 1);
                holder_transform =
                    make_householder(&lower_slice.iter().cloned().collect::<Vec<_>>());
            }

            if !holder_transform.is_ok() {
                return Err(Error::new(ErrorKind::DecompFailure,
                                      "Cannot compute QR decomposition."));
            } else {
                let mut holder_data = holder_transform.unwrap().into_vec();

                // This bit is inefficient
                // using for now as we'll swap to lapack eventually.
                let mut h_full_data = Vec::with_capacity(m * m);

                for j in 0..m {
                    let mut row_data: Vec<T>;
                    if j < i {
                        row_data = vec![T::zero(); m];
                        row_data[j] = T::one();
                        h_full_data.extend(row_data);
                    } else {
                        row_data = vec![T::zero(); i];
                        h_full_data.extend(row_data);
                        h_full_data.extend(holder_data.drain(..m - i));
                    }
                }

                let h = Matrix::new(m, m, h_full_data);

                q = q * &h;
                r = h * &r;
            }
        }

        Ok((q, r))
    }

    /// Converts matrix to bidiagonal form
    ///
    /// Returns (B, U, V), where B is bidiagonal and `self = U B V_T`.
    ///
    /// Note that if `self` has `self.rows() > self.cols()` the matrix will
    /// be transposed and then reduced - this will lead to a sub-diagonal instead
    /// of super-diagonal.
    ///
    /// # Failures
    ///
    /// - The matrix cannot be reduced to bidiagonal form.
    fn bidiagonal_decomp(self) -> Result<(Matrix<T>, Matrix<T>, Matrix<T>), Error>
        where T: Any + Float,
    {
        let mut flipped = false;

        let mut self_m = if self.rows() < self.cols() {
            flipped = true;
            self.transpose()
        } else {
            self.into_matrix()
        };

        let m = self_m.rows;
        let n = self_m.cols;

        let mut u = Matrix::identity(m);
        let mut v = Matrix::identity(n);

        for k in 0..n {
            let h_holder: Matrix<T>;
            {
                let lower_slice = MatrixSlice::from_matrix(&self_m, [k, k], m - k, 1);
                h_holder = try!(make_householder(&lower_slice.iter()
                        .cloned()
                        .collect::<Vec<_>>())
                    .map_err(|_| {
                        Error::new(ErrorKind::DecompFailure, "Cannot compute bidiagonal form.")
                    }));
            }

            {
                // Apply householder on the left to kill under diag.
                let lower_self_m_block = MatrixSliceMut::from_matrix(&mut self_m, [k, k], m - k, n - k);
                let transformed_self_m = &h_holder * &lower_self_m_block;
                lower_self_m_block.set_to(transformed_self_m.as_slice());
                let lower_u_block = MatrixSliceMut::from_matrix(&mut u, [0, k], m, m - k);
                let transformed_u = &lower_u_block * h_holder;
                lower_u_block.set_to(transformed_u.as_slice());
            }

            if k < n - 2 {
                let row: &[T];
                unsafe {
                    // Get the kth row from column k+1 to end.
                    row = slice::from_raw_parts(self_m.data
                                                    .as_ptr()
                                                    .offset((k * self_m.cols + k + 1) as isize),
                                                n - k - 1);
                }

                let row_h_holder = try!(make_householder(row).map_err(|_| {
                    Error::new(ErrorKind::DecompFailure, "Cannot compute bidiagonal form.")
                }));

                {
                    // Apply householder on the right to kill right of super diag.
                    let lower_self_m_block =
                        MatrixSliceMut::from_matrix(&mut self_m, [k, k + 1], m - k, n - k - 1);

                    let transformed_self_m = &lower_self_m_block * &row_h_holder;
                    lower_self_m_block.set_to(transformed_self_m.as_slice());
                    let lower_v_block =
                        MatrixSliceMut::from_matrix(&mut v, [0, k + 1], n, n - k - 1);
                    let transformed_v = &lower_v_block * row_h_holder;
                    lower_v_block.set_to(transformed_v.as_slice());

                }
            }
        }

        // Trim off the zerod blocks.
        self_m.data.truncate(n * n);
        self_m.rows = n;
        u = MatrixSlice::from_matrix(&u, [0, 0], m, n).into_matrix();

        if flipped {
            Ok((self_m.transpose(), v, u))
        } else {
            Ok((self_m, u, v))
        }

    }

    /// Singular Value Decomposition
    ///
    /// Computes the SVD using Golub-Reinsch algorithm.
    ///
    /// Returns Σ, U, V where self = U Σ V<sup>T</sup>.
    ///
    /// # Failures
    ///
    /// This function may fail in some cases. The current decomposition whilst being
    /// efficient is fairly basic. Hopefully the algorithm can be made not to fail in the near future.
    fn svd(self) -> Result<(Matrix<T>, Matrix<T>, Matrix<T>), Error>
        where T: Any + Float + Signed,
    {
        let mut flipped = false;

        // The algorithm assumes rows > cols. If this is not the case we transpose and fix later.
        let self_m = if self.rows() < self.cols() {
            flipped = true;
            self.transpose()
        } else {
            self.into_matrix()
        };

        let n = self_m.cols;

        // Get the bidiagonal decomposition
        let (mut b, mut u, mut v) = try!(self_m.bidiagonal_decomp()
            .map_err(|_| Error::new(ErrorKind::DecompFailure, "Could not compute SVD.")));

        loop {
            // Values to count the size of lower diagonal block
            let mut q = 0;
            let mut on_lower = true;

            // Values to count top block
            let mut p = 0;
            let mut on_middle = false;

            // Iterate through and hard set the super diag if converged
            for i in (0..n - 1).rev() {
                let (b_ii, b_sup_diag, diag_abs_sum): (T, T, T);
                unsafe {
                    b_ii = *b.get_unchecked([i, i]);
                    b_sup_diag = b.get_unchecked([i, i + 1]).abs();
                    diag_abs_sum = T::min_positive_value() *
                                   (b_ii.abs() + *b.get_unchecked([i + 1, i + 1]));
                }
                if b_sup_diag <= diag_abs_sum {
                    // Adjust q or p to define boundaries of sup-diagonal box
                    if on_lower {
                        q += 1;
                    } else if on_middle {
                        on_middle = false;
                        p = i + 1;
                    }
                    unsafe {
                        *b.get_unchecked_mut([i, i + 1]) = T::zero();
                    }
                } else {
                    if on_lower {
                        // No longer on the lower diagonal
                        on_middle = true;
                        on_lower = false;
                    }
                }
            }

            // We have converged!
            if q == n - 1 {
                break;
            }

            // Zero off diagonals if needed.
            for i in p..n - q - 1 {
                let (b_ii, b_sup_diag): (T, T);
                unsafe {
                    b_ii = *b.get_unchecked([i, i]);
                    b_sup_diag = *b.get_unchecked([i, i + 1]);
                }

                if b_ii.abs() < T::min_positive_value() {
                    let (c, s) = givens_rot(b_ii, b_sup_diag);
                    let givens = Matrix::new(2, 2, vec![c, s, -s, c]);
                    let b_i = MatrixSliceMut::from_matrix(&mut b, [i, i], 1, 2);
                    let zerod_line = &b_i * givens;

                    b_i.set_to(zerod_line.as_slice());
                }
            }

            // Apply Golub-Kahan svd step
            unsafe {
                try!(golub_kahan_svd_step(&mut b, &mut u, &mut v, p, q)
                    .map_err(|_| Error::new(ErrorKind::DecompFailure, "Could not compute SVD.")));
            }
        }

        if flipped {
            Ok((b.transpose(), v, u))
        } else {
            Ok((b, u, v))
        }

    }

    /// Returns H, where H is the upper hessenberg form.
    ///
    /// If the transformation matrix is also required, you should
    /// use `upper_hess_decomp`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let a = Matrix::new(4,4,vec![2.,0.,1.,1.,2.,0.,1.,2.,1.,2.,0.,0.,2.,0.,1.,1.]);
    /// let h = a.upper_hessenberg();
    ///
    /// println!("{:?}", h.expect("Could not get upper Hessenberg form.").data());
    /// ```
    ///
    /// # Panics
    ///
    /// - The matrix is not square.
    ///
    /// # Failures
    ///
    /// - The matrix cannot be reduced to upper hessenberg form.
    fn upper_hessenberg(&self) -> Result<Matrix<T>, Error>
        where T: Any + Float + Signed,
    {
        let n = self.rows();
        assert!(n == self.cols(),
                "Matrix must be square to produce upper hessenberg.");

        let mut self_m = self.as_matrix();

        for i in 0..n - 2 {
            let h_holder_vec: Matrix<T>;
            {
                let lower_slice = MatrixSlice::from_matrix(&self_m, [i + 1, i], n - i - 1, 1);
                // Try to get the house holder transform - else map error and pass up.
                h_holder_vec = try!(make_householder_vec(&lower_slice.iter()
                        .cloned()
                        .collect::<Vec<_>>())
                    .map_err(|_| {
                        Error::new(ErrorKind::DecompFailure,
                                   "Cannot compute upper Hessenberg form.")
                    }));
            }

            {
                // Apply holder on the left
                let mut block =
                    MatrixSliceMut::from_matrix(&mut self_m, [i + 1, i], n - i - 1, n - i);
                block -= &h_holder_vec * (h_holder_vec.transpose() * &block) *
                         (T::one() + T::one());
            }

            {
                // Apply holder on the right
                let mut block = MatrixSliceMut::from_matrix(&mut self_m, [0, i + 1], n, n - i - 1);
                block -= (&block * &h_holder_vec) * h_holder_vec.transpose() *
                         (T::one() + T::one());
            }

        }

        // Enforce upper hessenberg
        for i in 0..self_m.cols - 2 {
            for j in i + 2..self_m.rows {
                unsafe {
                    *self_m.get_unchecked_mut([j, i]) = T::zero();
                }
            }
        }

        Ok(self_m)
    }

    /// Returns (U,H), where H is the upper hessenberg form
    /// and U is the unitary transform matrix.
    ///
    /// Note: The current transform matrix seems broken...
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::slice::BaseSlice;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let a = Matrix::new(3,3,vec![1.,2.,3.,4.,5.,6.,7.,8.,9.]);
    ///
    /// // u is the transform, h is the upper hessenberg form.
    /// let (u,h) = a.clone().upper_hess_decomp().expect("This matrix should decompose!");
    ///
    /// println!("The hess : {:?}", h.data());
    /// println!("Manual hess : {:?}", (u.transpose() * a * u).data());
    /// ```
    ///
    /// # Panics
    ///
    /// - The matrix is not square.
    ///
    /// # Failures
    ///
    /// - The matrix cannot be reduced to upper hessenberg form.
    fn upper_hess_decomp(self) -> Result<(Matrix<T>, Matrix<T>), Error>
        where T: Any + Float + Signed,
    {
        let n = self.rows();
        assert!(n == self.cols(),
                "Matrix must be square to produce upper hessenberg.");

        // First we form the transformation.
        let mut transform = Matrix::identity(n);
        let self_m = self.into_matrix();

        for i in (0..n - 2).rev() {
            let h_holder_vec: Matrix<T>;
            {
                let lower_slice = MatrixSlice::from_matrix(&self_m, [i + 1, i], n - i - 1, 1);
                h_holder_vec = try!(make_householder_vec(&lower_slice.iter()
                        .cloned()
                        .collect::<Vec<_>>())
                    .map_err(|_| {
                        Error::new(ErrorKind::DecompFailure, "Could not compute eigenvalues.")
                    }));
            }

            let mut trans_block =
                MatrixSliceMut::from_matrix(&mut transform, [i + 1, i + 1], n - i - 1, n - i - 1);
            trans_block -= &h_holder_vec * (h_holder_vec.transpose() * &trans_block) *
                           (T::one() + T::one());
        }

        // Now we reduce to upper hessenberg
        Ok((transform, try!(self_m.upper_hessenberg())))
    }

    /// Eigenvalues of a square matrix.
    ///
    /// Returns a Vec of eigenvalues.
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let a = Matrix::new(4,4, (1..17).map(|v| v as f64).collect::<Vec<f64>>());
    /// let e = a.eigenvalues().expect("We should be able to compute these eigenvalues!");
    /// println!("{:?}", e);
    /// ```
    ///
    /// # Panics
    ///
    /// - The matrix is not square.
    ///
    /// # Failures
    ///
    /// - Eigenvalues cannot be computed.
    fn eigenvalues(&self) -> Result<Vec<T>, Error>
        where T: Any + Float + Signed,
    {
        let n = self.rows();
        assert!(n == self.cols(),
                "Matrix must be square for eigenvalue computation.");

        match n {
            1 => Ok(vec![*self.iter().next().unwrap()]),
            2 => direct_2_by_2_eigenvalues(self),
            _ => francis_shift_eigenvalues(self),
        }
    }

    /// Eigendecomposition of a square matrix.
    ///
    /// Returns a Vec of eigenvalues, and a matrix with eigenvectors as the columns.
    ///
    /// The eigenvectors are only gauranteed to be correct if the matrix is real-symmetric.
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let a = Matrix::new(3,3,vec![3.,2.,4.,2.,0.,2.,4.,2.,3.]);
    ///
    /// let (e, m) = a.eigendecomp().expect("We should be able to compute this eigendecomp!");
    /// println!("{:?}", e);
    /// println!("{:?}", m.data());
    /// ```
    ///
    /// # Panics
    ///
    /// - The matrix is not square.
    ///
    /// # Failures
    ///
    /// - The eigen decomposition can not be computed.
    fn eigendecomp(&self) -> Result<(Vec<T>, Matrix<T>), Error>
        where T: Any + Float + Signed,
    {
        let n = self.rows();
        assert!(n == self.cols(), "Matrix must be square for eigendecomp.");

        match n {
            1 => Ok((vec![*self.iter().next().unwrap()], Matrix::new(1, 1, vec![T::one()]))),
            2 => direct_2_by_2_eigendecomp(self),
            _ => francis_shift_eigendecomp(self),
        }
    }

    /// Computes L, U, and P for LUP decomposition.
    ///
    /// Returns L,U, and P respectively.
    ///
    /// # Examples
    ///
    /// ```
    /// use rulinalg::matrix::Matrix;
    /// use rulinalg::matrix::decomposition::Decomposition;
    ///
    /// let a = Matrix::new(3,3, vec![1.0,2.0,0.0,
    ///                               0.0,3.0,4.0,
    ///                               5.0, 1.0, 2.0]);
    ///
    /// let (l,u,p) = a.lup_decomp().expect("This matrix should decompose!");
    /// ```
    ///
    /// # Panics
    ///
    /// - Matrix is not square.
    ///
    /// # Failures
    ///
    /// - Matrix cannot be LUP decomposed.
    fn lup_decomp(&self) -> Result<(Matrix<T>, Matrix<T>, Matrix<T>), Error>
        where T: Any + Copy + One + Zero + Neg<Output=T> + Add<T, Output=T> + 
                 Mul<T, Output=T> + Sub<T, Output=T> + Div<T, Output=T> + PartialOrd,
              for <'a> &'a Matrix<T>: Mul<&'a Self, Output=Matrix<T>>,
    {
        let n = self.cols();
        assert!(self.rows() == n, "Matrix must be square for LUP decomposition.");

        let mut l = Matrix::<T>::zeros(n, n);
        let mut u = Matrix::<T>::zeros(n, n);

        let mt = self.transpose();

        let mut p = Matrix::<T>::identity(n);

        // Compute the permutation matrix
        for i in 0..n {
            let (row,_) = utils::argmax(&mt.data[i*(n+1)..(i+1)*n]);

            if row != 0 {
                for j in 0..n {
                    p.data.swap(i*n + j, row*n+j)
                }
            }
        }

        let a_2 = &p * self;

        for i in 0..n {
            l.data[i*(n+1)] = T::one();

            for j in 0..i+1 {
                let mut s1 = T::zero();

                for k in 0..j {
                    s1 = s1 + l.data[j*n + k] * u.data[k*n + i];
                }

                u.data[j*n + i] = a_2[[j,i]] - s1;
            }

            for j in i..n {
                let mut s2 = T::zero();

                for k in 0..i {
                    s2 = s2 + l.data[j*n + k] * u.data[k*n + i];
                }

                let denom = u[[i,i]];

                if denom == T::zero() {
                    return Err(Error::new(ErrorKind::DecompFailure,
                        "Matrix could not be LUP decomposed."));
                }
                l.data[j*n + i] = (a_2[[j,i]] - s2) / denom;
            }

        }

        Ok((l,u,p))
    }
}


impl<T> Decomposition<T> for Matrix<T> {}
impl<'a, T> Decomposition<T> for MatrixSlice<'a, T> {}
impl<'a, T> Decomposition<T> for MatrixSliceMut<'a, T> {}

/// Compute the cos and sin values for the givens rotation.
///
/// Returns a tuple (c, s).
fn givens_rot<T: Any + Float>(a: T, b: T) -> (T, T) {
    let r = a.hypot(b);

    (a / r, -b / r)
}

fn make_householder<T: Any + Float>(column: &[T]) -> Result<Matrix<T>, Error> {
    let size = column.len();

    if size == 0 {
        return Err(Error::new(ErrorKind::InvalidArg,
                              "Column for householder transform cannot be empty."));
    }

    let denom = column[0] + column[0].signum() * utils::dot(column, column).sqrt();

    if denom == T::zero() {
        return Err(Error::new(ErrorKind::DecompFailure,
                              "Cannot produce househoulder transform from column as first \
                               entry is 0."));
    }

    let mut v = column.into_iter().map(|&x| x / denom).collect::<Vec<T>>();
    // Ensure first element is fixed to 1.
    v[0] = T::one();
    let v = Vector::new(v);
    let v_norm_sq = v.dot(&v);

    let v_vert = Matrix::new(size, 1, v.data().clone());
    let v_hor = Matrix::new(1, size, v.into_vec());
    Ok(Matrix::<T>::identity(size) - (v_vert * v_hor) * ((T::one() + T::one()) / v_norm_sq))
}

fn make_householder_vec<T: Any + Float>(column: &[T]) -> Result<Matrix<T>, Error> {
    let size = column.len();

    if size == 0 {
        return Err(Error::new(ErrorKind::InvalidArg,
                              "Column for householder transform cannot be empty."));
    }

    let denom = column[0] + column[0].signum() * utils::dot(column, column).sqrt();

    if denom == T::zero() {
        return Err(Error::new(ErrorKind::DecompFailure,
                              "Cannot produce househoulder transform from column as first \
                               entry is 0."));
    }

    let mut v = column.into_iter().map(|&x| x / denom).collect::<Vec<T>>();
    // Ensure first element is fixed to 1.
    v[0] = T::one();
    let v = Matrix::new(size, 1, v);

    Ok(&v / v.norm())
}

/// This function is unsafe as it makes assumptions about the dimensions
/// of the inputs matrices and does not check them. As a result if misused
/// this function can call `get_unchecked` on invalid indices.
unsafe fn golub_kahan_svd_step<T>(b: &mut Matrix<T>,
                                  u: &mut Matrix<T>,
                                  v: &mut Matrix<T>,
                                  p: usize,
                                  q: usize)
                               -> Result<(), Error>
    where T: Any + Float + Signed,
{
    let n = b.rows();

    // C is the lower, right 2x2 square of aTa, where a is the
    // middle block of b (between p and n-q).
    //
    // Computed as xTx + yTy, where y is the bottom 2x2 block of a
    // and x are the two columns above it within a.
    let c: Matrix<T>;
    {
        let y = MatrixSlice::from_matrix(&b, [n - q - 2, n - q - 2], 2, 2).into_matrix();
        if n - q - p - 2 > 0 {
            let x = MatrixSlice::from_matrix(&b, [p, n - q - 2], n - q - p - 2, 2);
            c = x.into_matrix().transpose() * x + y.transpose() * y;
        } else {
            c = y.transpose() * y;
        }
    }

    let c_eigs = try!(c.eigenvalues());

    // Choose eigenvalue closes to c[1,1].
    let lambda: T;
    if (c_eigs[0] - *c.get_unchecked([1, 1])).abs() <
       (c_eigs[1] - *c.get_unchecked([1, 1])).abs() {
        lambda = c_eigs[0];
    } else {
        lambda = c_eigs[1];
    }

    let b_pp = *b.get_unchecked([p, p]);
    let mut alpha = (b_pp * b_pp) - lambda;
    let mut beta = b_pp * *b.get_unchecked([p, p + 1]);
    for k in p..n - q - 1 {
        // Givens rot on columns k and k + 1
        let (c, s) = givens_rot(alpha, beta);
        let givens_mat = Matrix::new(2, 2, vec![c, s, -s, c]);

        {
            // Pick the rows from b to be zerod.
            let b_block = MatrixSliceMut::from_matrix(b,
                                                      [k.saturating_sub(1), k],
                                                      cmp::min(3, n - k.saturating_sub(1)),
                                                      2);
            let transformed = &b_block * &givens_mat;
            b_block.set_to(transformed.as_slice());

            let v_block = MatrixSliceMut::from_matrix(v, [0, k], n, 2);
            let transformed = &v_block * &givens_mat;
            v_block.set_to(transformed.as_slice());
        }

        alpha = *b.get_unchecked([k, k]);
        beta = *b.get_unchecked([k + 1, k]);

        let (c, s) = givens_rot(alpha, beta);
        let givens_mat = Matrix::new(2, 2, vec![c, -s, s, c]);

        {
            // Pick the columns from b to be zerod.
            let b_block = MatrixSliceMut::from_matrix(b, [k, k], 2, cmp::min(3, n - k));
            let transformed = &givens_mat * &b_block;
            b_block.set_to(transformed.as_slice());

            let m = u.rows();
            let u_block = MatrixSliceMut::from_matrix(u, [0, k], m, 2);
            let transformed = &u_block * givens_mat.transpose();
            u_block.set_to(transformed.as_slice());
        }

        if k + 2 < n - q {
            alpha = *b.get_unchecked([k, k + 1]);
            beta = *b.get_unchecked([k, k + 2]);
        }
    }
    Ok(())
}

fn balance_matrix<T, M>(self_m: &mut M)
    where T: Any + Float + Signed,
          M: BaseSliceMut<T>
{
    let n = self_m.rows();
    let radix = T::one() + T::one();

    debug_assert!(n == self_m.cols(),
                  "Matrix must be square to produce balance matrix.");

    let mut d = Matrix::<T>::identity(n);
    let mut converged = false;

    while !converged {
        converged = true;

        for i in 0..n {
            let mut c = self_m.select_cols(&[i]).norm();
            let mut r = self_m.select_rows(&[i]).norm();

            let s = c * c + r * r;
            let mut f = T::one();

            while c < r / radix {
                c = c * radix;
                r = r / radix;
                f = f * radix;
            }

            while c >= r * radix {
                c = c / radix;
                r = r * radix;
                f = f / radix;
            }

            if (c * c + r * r) < cast::<f64, T>(0.95).unwrap() * s {
                converged = false;
                d.data[i * (self_m.cols() + 1)] = f * d.data[i * (self_m.cols() + 1)];

                for j in 0..n {
                    unsafe {
                        *self_m.get_unchecked_mut([j, i]) = f * *self_m.get_unchecked([j, i]);
                        *self_m.get_unchecked_mut([i, j]) = *self_m.get_unchecked([i, j]) / f;
                    }
                }
            }
        }
    }
}

fn direct_2_by_2_eigenvalues<T, M>(self_m: &M) -> Result<Vec<T>, Error>
    where T: Any + Float + Signed,
          M: BaseSlice<T>
{
    let data = {
        let mut iter = self_m.iter();
        [
            *iter.next().unwrap(),
            *iter.next().unwrap(),
            *iter.next().unwrap(),
            *iter.next().unwrap()
        ]
    };

    // The characteristic polynomial of a 2x2 matrix A is
    // λ² − (a₁₁ + a₂₂)λ + (a₁₁a₂₂ − a₁₂a₂₁);
    // the quadratic formula suffices.
    let tr = data[0] + data[3];
    let det = data[0] * data[3] - data[1] * data[2];

    let two = T::one() + T::one();
    let four = two + two;

    let discr = tr * tr - four * det;

    if discr < T::zero() {
        Err(Error::new(ErrorKind::DecompFailure,
                       "Matrix has complex eigenvalues. Currently unsupported, sorry!"))
    } else {
        let discr_root = discr.sqrt();
        Ok(vec![(tr - discr_root) / two, (tr + discr_root) / two])
    }

}

fn francis_shift_eigenvalues<T, M>(self_m: &M) -> Result<Vec<T>, Error>
    where T: Any + Float + Signed,
          M: Decomposition<T>,
{
    let n = self_m.rows();
    debug_assert!(n > 2,
                  "Francis shift only works on matrices greater than 2x2.");
    debug_assert!(n == self_m.cols(), "Matrix must be square for Francis shift.");

    let mut h = try!(self_m.upper_hessenberg()
        .map_err(|_| Error::new(ErrorKind::DecompFailure, "Could not compute eigenvalues.")));
    balance_matrix(&mut h);

    // The final index of the active matrix
    let mut p = n - 1;

    let eps = cast::<f64, T>(1e-20).expect("Failed to cast value for convergence check.");

    while p > 1 {
        let q = p - 1;
        let s = h[[q, q]] + h[[p, p]];
        let t = h[[q, q]] * h[[p, p]] - h[[q, p]] * h[[p, q]];

        let mut x = h[[0, 0]] * h[[0, 0]] + h[[0, 1]] * h[[1, 0]] - h[[0, 0]] * s + t;
        let mut y = h[[1, 0]] * (h[[0, 0]] + h[[1, 1]] - s);
        let mut z = h[[1, 0]] * h[[2, 1]];

        for k in 0..p - 1 {
            let r = cmp::max(1, k) - 1;

            let householder = try!(make_householder(&[x, y, z]).map_err(|_| {
                Error::new(ErrorKind::DecompFailure, "Could not compute eigenvalues.")
            }));

            {
                // Apply householder transformation to block (on the left)
                let h_block = MatrixSliceMut::from_matrix(&mut h, [k, r], 3, n - r);
                let transformed = &householder * &h_block;
                h_block.set_to(transformed.as_slice());
            }

            let r = cmp::min(k + 4, p + 1);

            {
                // Apply householder transformation to the block (on the right)
                let h_block = MatrixSliceMut::from_matrix(&mut h, [0, k], r, 3);
                let transformed = &h_block * householder.transpose();
                h_block.set_to(transformed.as_slice());
            }

            x = h[[k + 1, k]];
            y = h[[k + 2, k]];

            if k < p - 2 {
                z = h[[k + 3, k]];
            }
        }

        let (c, s) = givens_rot(x, y);
        let givens_mat = Matrix::new(2, 2, vec![c, -s, s, c]);

        {
            // Apply Givens rotation to the block (on the left)
            let h_block = MatrixSliceMut::from_matrix(&mut h, [q, p - 2], 2, n - p + 2);
            let transformed = &givens_mat * &h_block;
            h_block.set_to(transformed.as_slice());
        }

        {
            // Apply Givens rotation to block (on the right)
            let h_block = MatrixSliceMut::from_matrix(&mut h, [0, q], p + 1, 2);
            let transformed = &h_block * givens_mat.transpose();
            h_block.set_to(transformed.as_slice());
        }

        // Check for convergence
        if abs(h[[p, q]]) < eps * (abs(h[[q, q]]) + abs(h[[p, p]])) {
            h.data[p * h.cols + q] = T::zero();
            p -= 1;
        } else if abs(h[[p - 1, q - 1]]) < eps * (abs(h[[q - 1, q - 1]]) + abs(h[[q, q]])) {
            h.data[(p - 1) * h.cols + q - 1] = T::zero();
            p -= 2;
        }
    }

    Ok(h.diag().into_vec())
}

fn direct_2_by_2_eigendecomp<T, M>(self_m: &M) -> Result<(Vec<T>, Matrix<T>), Error>
    where T: Any + Float + Signed,
          M: Decomposition<T>
{
    let eigenvalues = try!(self_m.eigenvalues());
    let data = {
        let mut iter = self_m.iter();
        [
            *iter.next().unwrap(),
            *iter.next().unwrap(),
            *iter.next().unwrap(),
            *iter.next().unwrap()
        ]
    };

    // Thanks to
    // http://www.math.harvard.edu/archive/21b_fall_04/exhibits/2dmatrices/index.html
    // for this characterization—
    if data[2] != T::zero() {
        let decomp_data = vec![eigenvalues[0] - data[3],
                               eigenvalues[1] - data[3],
                               data[2],
                               data[2]];
        Ok((eigenvalues, Matrix::new(2, 2, decomp_data)))
    } else if data[1] != T::zero() {
        let decomp_data = vec![data[1],
                               data[1],
                               eigenvalues[0] - data[0],
                               eigenvalues[1] - data[0]];
        Ok((eigenvalues, Matrix::new(2, 2, decomp_data)))
    } else {
        Ok((eigenvalues, Matrix::new(2, 2, vec![T::one(), T::zero(), T::zero(), T::one()])))
    }
}

fn francis_shift_eigendecomp<T, M>(self_m: &M) -> Result<(Vec<T>, Matrix<T>), Error>
    where T: Any + Float + Signed,
          M: BaseSlice<T>
{
    let n = self_m.rows();
    debug_assert!(n > 2,
                  "Francis shift only works on matrices greater than 2x2.");
    debug_assert!(n == self_m.cols(), "Matrix must be square for Francis shift.");

    let self_m = self_m.as_matrix();
    let (u, mut h) = try!(self_m.clone().upper_hess_decomp().map_err(|_| {
        Error::new(ErrorKind::DecompFailure,
                   "Could not compute eigen decomposition.")
    }));
    balance_matrix(&mut h);
    let mut transformation = Matrix::identity(n);

    // The final index of the active matrix
    let mut p = n - 1;

    let eps = cast::<f64, T>(1e-20).expect("Failed to cast value for convergence check.");

    while p > 1 {
        let q = p - 1;
        let s = h[[q, q]] + h[[p, p]];
        let t = h[[q, q]] * h[[p, p]] - h[[q, p]] * h[[p, q]];

        let mut x = h[[0, 0]] * h[[0, 0]] + h[[0, 1]] * h[[1, 0]] - h[[0, 0]] * s + t;
        let mut y = h[[1, 0]] * (h[[0, 0]] + h[[1, 1]] - s);
        let mut z = h[[1, 0]] * h[[2, 1]];

        for k in 0..p - 1 {
            let r = cmp::max(1, k) - 1;

            let householder = try!(make_householder(&[x, y, z]).map_err(|_| {
                Error::new(ErrorKind::DecompFailure,
                           "Could not compute eigen decomposition.")
            }));

            {
                // Apply householder transformation to block (on the left)
                let h_block = MatrixSliceMut::from_matrix(&mut h, [k, r], 3, n - r);
                let transformed = &householder * &h_block;
                h_block.set_to(transformed.as_slice());
            }

            let r = cmp::min(k + 4, p + 1);

            {
                // Apply householder transformation to the block (on the right)
                let h_block = MatrixSliceMut::from_matrix(&mut h, [0, k], r, 3);
                let transformed = &h_block * householder.transpose();
                h_block.set_to(transformed.as_slice());
            }

            {
                // Update the transformation matrix
                let trans_block =
                    MatrixSliceMut::from_matrix(&mut transformation, [0, k], n, 3);
                let transformed = &trans_block * householder.transpose();
                trans_block.set_to(transformed.as_slice());
            }

            x = h[[k + 1, k]];
            y = h[[k + 2, k]];

            if k < p - 2 {
                z = h[[k + 3, k]];
            }
        }

        let (c, s) = givens_rot(x, y);
        let givens_mat = Matrix::new(2, 2, vec![c, -s, s, c]);

        {
            // Apply Givens rotation to the block (on the left)
            let h_block = MatrixSliceMut::from_matrix(&mut h, [q, p - 2], 2, n - p + 2);
            let transformed = &givens_mat * &h_block;
            h_block.set_to(transformed.as_slice());
        }

        {
            // Apply Givens rotation to block (on the right)
            let h_block = MatrixSliceMut::from_matrix(&mut h, [0, q], p + 1, 2);
            let transformed = &h_block * givens_mat.transpose();
            h_block.set_to(transformed.as_slice());
        }

        {
            // Update the transformation matrix
            let trans_block = MatrixSliceMut::from_matrix(&mut transformation, [0, q], n, 2);
            let transformed = &trans_block * givens_mat.transpose();
            trans_block.set_to(transformed.as_slice());
        }

        // Check for convergence
        if abs(h[[p, q]]) < eps * (abs(h[[q, q]]) + abs(h[[p, p]])) {
            h.data[p * h.cols + q] = T::zero();
            p -= 1;
        } else if abs(h[[p - 1, q - 1]]) < eps * (abs(h[[q - 1, q - 1]]) + abs(h[[q, q]])) {
            h.data[(p - 1) * h.cols + q - 1] = T::zero();
            p -= 2;
        }
    }

    Ok((h.diag().into_vec(), u * transformation))
}


#[cfg(test)]
mod tests {
    use matrix::Matrix;
    use vector::Vector;
    use matrix::slice::BaseSlice;
    use matrix::decomposition::Decomposition;

    fn validate_bidiag(mat: &Matrix<f64>,
                       b: &Matrix<f64>,
                       u: &Matrix<f64>,
                       v: &Matrix<f64>,
                       upper: bool) {
        for (idx, row) in b.iter_rows().enumerate() {
            let pair_start = if upper {
                idx
            } else {
                idx.saturating_sub(1)
            };
            assert!(!row.iter().take(pair_start).any(|&x| x > 1e-10));
            assert!(!row.iter().skip(pair_start + 2).any(|&x| x > 1e-10));
        }

        let recovered = u * b * v.transpose();

        assert_eq!(recovered.rows(), mat.rows());
        assert_eq!(recovered.cols(), mat.cols());

        assert!(!mat.data()
            .iter()
            .zip(recovered.data().iter())
            .any(|(&x, &y)| (x - y).abs() > 1e-10));
    }

    #[test]
    fn test_bidiagonal_square() {
        let mat = Matrix::new(5,
                              5,
                              vec![1f64, 2.0, 3.0, 4.0, 5.0, 2.0, 4.0, 1.0, 2.0, 1.0, 3.0, 1.0,
                                   7.0, 1.0, 1.0, 4.0, 2.0, 1.0, -1.0, 3.0, 5.0, 1.0, 1.0, 3.0,
                                   2.0]);
        let (b, u, v) = mat.clone().bidiagonal_decomp().unwrap();
        validate_bidiag(&mat, &b, &u, &v, true);
    }

    #[test]
    fn test_bidiagonal_non_square() {
        let mat = Matrix::new(5,
                              3,
                              vec![1f64, 2.0, 3.0, 4.0, 5.0, 2.0, 4.0, 1.0, 2.0, 1.0, 3.0, 1.0,
                                   7.0, 1.0, 1.0]);
        let (b, u, v) = mat.clone().bidiagonal_decomp().unwrap();
        validate_bidiag(&mat, &b, &u, &v, true);

        let mat = Matrix::new(3,
                              5,
                              vec![1f64, 2.0, 3.0, 4.0, 5.0, 2.0, 4.0, 1.0, 2.0, 1.0, 3.0, 1.0,
                                   7.0, 1.0, 1.0]);
        let (b, u, v) = mat.clone().bidiagonal_decomp().unwrap();
        validate_bidiag(&mat, &b, &u, &v, false);
    }

    fn validate_svd(mat: &Matrix<f64>, b: &Matrix<f64>, u: &Matrix<f64>, v: &Matrix<f64>) {
        // b is diagonal (the singular values)
        for (idx, row) in b.iter_rows().enumerate() {
            assert!(!row.iter().take(idx).any(|&x| x > 1e-10));
            assert!(!row.iter().skip(idx + 1).any(|&x| x > 1e-10));
        }

        let recovered = u * b * v.transpose();

        assert_eq!(recovered.rows(), mat.rows());
        assert_eq!(recovered.cols(), mat.cols());

        assert!(!mat.data()
            .iter()
            .zip(recovered.data().iter())
            .any(|(&x, &y)| (x - y).abs() > 1e-10));
    }

    #[test]
    fn test_svd_non_square() {
        let mat = Matrix::new(5,
                              3,
                              vec![1f64, 2.0, 3.0, 4.0, 5.0, 2.0, 4.0, 1.0, 2.0, 1.0, 3.0, 1.0,
                                   7.0, 1.0, 1.0]);
        let (b, u, v) = mat.clone().svd().unwrap();

        validate_svd(&mat, &b, &u, &v);

        let mat = Matrix::new(3,
                              5,
                              vec![1f64, 2.0, 3.0, 4.0, 5.0, 2.0, 4.0, 1.0, 2.0, 1.0, 3.0, 1.0,
                                   7.0, 1.0, 1.0]);
        let (b, u, v) = mat.clone().svd().unwrap();

        validate_svd(&mat, &b, &u, &v);
    }

    #[test]
    fn test_svd_square() {
        let mat = Matrix::new(5,
                              5,
                              vec![1f64, 2.0, 3.0, 4.0, 5.0, 2.0, 4.0, 1.0, 2.0, 1.0, 3.0, 1.0,
                                   7.0, 1.0, 1.0, 4.0, 2.0, 1.0, -1.0, 3.0, 5.0, 1.0, 1.0, 3.0,
                                   2.0]);
        let (b, u, v) = mat.clone().svd().unwrap();
        validate_svd(&mat, &b, &u, &v);
    }

    #[test]
    fn test_1_by_1_matrix_eigenvalues() {
        let a = Matrix::new(1, 1, vec![3.]);
        assert_eq!(vec![3.], a.eigenvalues().unwrap());
    }

    #[test]
    fn test_2_by_2_matrix_eigenvalues() {
        let a = Matrix::new(2, 2, vec![1., 2., 3., 4.]);
        // characteristic polynomial is λ² − 5λ − 2 = 0
        assert_eq!(vec![(5. - (33.0f32).sqrt()) / 2., (5. + (33.0f32).sqrt()) / 2.],
                   a.eigenvalues().unwrap());
    }

    #[test]
    fn test_2_by_2_matrix_zeros_eigenvalues() {
        let a = Matrix::new(2, 2, vec![0.; 4]);
        // characteristic polynomial is λ² = 0
        assert_eq!(vec![0.0, 0.0], a.eigenvalues().unwrap());
    }

    #[test]
    fn test_2_by_2_matrix_complex_eigenvalues() {
        // This test currently fails - complex eigenvalues would be nice though!
        let a = Matrix::new(2, 2, vec![1.0, -3.0, 1.0, 1.0]);
        // characteristic polynomial is λ² − λ + 4 = 0

        // Decomposition will fail
        assert!(a.eigenvalues().is_err());
    }

    #[test]
    fn test_2_by_2_matrix_eigendecomp() {
        let a = Matrix::new(2, 2, vec![20., 4., 20., 16.]);
        let (eigenvals, eigenvecs) = a.eigendecomp().unwrap();

        let lambda_1 = eigenvals[0];
        let lambda_2 = eigenvals[1];

        let v1 = Vector::new(vec![eigenvecs[[0, 0]], eigenvecs[[1, 0]]]);
        let v2 = Vector::new(vec![eigenvecs[[0, 1]], eigenvecs[[1, 1]]]);

        let epsilon = 0.00001;
        assert!((&a * &v1 - &v1 * lambda_1).into_vec().iter().all(|&c| c < epsilon));
        assert!((&a * &v2 - &v2 * lambda_2).into_vec().iter().all(|&c| c < epsilon));
    }

    #[test]
    fn test_3_by_3_eigenvals() {
        let a = Matrix::new(3, 3, vec![17f64, 22., 27., 22., 29., 36., 27., 36., 45.]);

        let eigs = a.eigenvalues().unwrap();

        let eig_1 = 90.4026;
        let eig_2 = 0.5973;
        let eig_3 = 0.0;

        assert!(eigs.iter().any(|x| (x - eig_1).abs() < 1e-4));
        assert!(eigs.iter().any(|x| (x - eig_2).abs() < 1e-4));
        assert!(eigs.iter().any(|x| (x - eig_3).abs() < 1e-4));
    }

    #[test]
    fn test_5_by_5_eigenvals() {
        let a = Matrix::new(5,
                            5,
                            vec![1f64, 2.0, 3.0, 4.0, 5.0, 2.0, 4.0, 1.0, 2.0, 1.0, 3.0, 1.0,
                                 7.0, 1.0, 1.0, 4.0, 2.0, 1.0, -1.0, 3.0, 5.0, 1.0, 1.0, 3.0, 2.0]);

        let eigs = a.eigenvalues().unwrap();

        let eig_1 = 12.174;
        let eig_2 = 5.2681;
        let eig_3 = -4.4942;
        let eig_4 = 2.9279;
        let eig_5 = -2.8758;

        assert!(eigs.iter().any(|x| (x - eig_1).abs() < 1e-4));
        assert!(eigs.iter().any(|x| (x - eig_2).abs() < 1e-4));
        assert!(eigs.iter().any(|x| (x - eig_3).abs() < 1e-4));
        assert!(eigs.iter().any(|x| (x - eig_4).abs() < 1e-4));
        assert!(eigs.iter().any(|x| (x - eig_5).abs() < 1e-4));
    }

    #[test]
    #[should_panic]
    fn test_non_square_cholesky() {
        let a = Matrix::new(2, 3, vec![1.0; 6]);

        let _ = a.cholesky();
    }

    #[test]
    #[should_panic]
    fn test_non_square_upper_hessenberg() {
        let a = Matrix::new(2, 3, vec![1.0; 6]);

        let _ = a.upper_hessenberg();
    }

    #[test]
    #[should_panic]
    fn test_non_square_upper_hess_decomp() {
        let a = Matrix::new(2, 3, vec![1.0; 6]);

        let _ = a.upper_hess_decomp();
    }

    #[test]
    #[should_panic]
    fn test_non_square_eigenvalues() {
        let a = Matrix::new(2, 3, vec![1.0; 6]);

        let _ = a.eigenvalues();
    }

    #[test]
    #[should_panic]
    fn test_non_square_eigendecomp() {
        let a = Matrix::new(2, 3, vec![1.0; 6]);

        let _ = a.eigendecomp();
    }

    #[test]
    #[should_panic]
    fn test_non_square_lup_decomp() {
        let a = Matrix::new(2, 3, vec![1.0; 6]);

        let _ = a.lup_decomp();
    }
}
