'use client';

import { useState, useEffect, Suspense } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import Link from 'next/link';
// Registration not available in local/bundled app (no auth backend)

function UserRegisterPageContent() {
  const router = useRouter();
  const searchParams = useSearchParams();
  
  // Get referral parameters from URL
  const referralJoinCode = searchParams.get('referral');
  const referralEmail = searchParams.get('email');
  
  // Decode email parameter and handle special characters
  const decodedEmail = referralEmail ? decodeURIComponent(referralEmail) : null;
  
  const [formData, setFormData] = useState({
    name: '',
    email: decodedEmail || '',
    password: '',
    confirmPassword: '',
    join_code: referralJoinCode || ''
  });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  
  // Determine if fields should be editable based on referral parameters
  const isReferralLink = referralJoinCode && decodedEmail;
  const isEmailEditable = !decodedEmail;
  const isJoinCodeEditable = !referralJoinCode;

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setFormData({
      ...formData,
      [e.target.name]: e.target.value
    });
    if (error) setError('');
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    // Validation
    if (formData.password !== formData.confirmPassword) {
      setError('Passwords do not match');
      return;
    }

    if (formData.password.length < 6) {
      setError('Password must be at least 6 characters long');
      return;
    }

    if (!formData.email.includes('@')) {
      setError('Please enter a valid email address');
      return;
    }

    if (!formData.join_code.trim()) {
      setError('Join code is required');
      return;
    }

    // Additional validation for referral links
    if (isReferralLink) {
      if (!decodedEmail || !referralJoinCode) {
        setError('Invalid referral link. Please contact your administrator.');
        return;
      }
      
      // Ensure the email in the form matches the referral email
      if (formData.email !== decodedEmail) {
        setError('Email address cannot be changed for referral registrations');
        return;
      }
      
      // Ensure the join code in the form matches the referral join code
      if (formData.join_code !== referralJoinCode) {
        setError('Join code cannot be changed for referral registrations');
        return;
      }
    }

    setLoading(true);
    setError('');

    try {
      // For local bundled app, registration is not available
      throw new Error('Registration is not available in the local bundled app');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Registration failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 px-6 py-12">
      <div className="w-full max-w-md">
        <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-10">
          <h1 className="text-3xl font-light text-gray-900 mb-2 text-center">
            {isReferralLink ? 'Complete Signup' : 'Create User Account'}
          </h1>
          {!isReferralLink && (
            <p className="text-sm text-gray-500 mb-8 text-center">
              Or{' '}
              <Link href="/register/business" className="text-blue-600 font-medium hover:text-blue-700">
                register for a business account
              </Link>
            </p>
          )}

          {isReferralLink && (
            <div className="mb-6 p-4 bg-blue-50 border border-blue-300 rounded-md flex gap-2">
              <svg className="flex-shrink-0 w-5 h-5 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 8l7.89 4.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
              </svg>
              <div className="flex-1">
                <p className="font-semibold text-blue-900 mb-1 text-sm">
                  You&apos;ve been invited to join an organization!
                </p>
                <p className="text-xs text-blue-700">
                  Your email and join code have been pre-filled for you.
                </p>
              </div>
            </div>
          )}
          
          {error && (
            <div className="mb-6 p-4 bg-red-50 border border-red-200 rounded text-sm text-red-700">
              {error}
            </div>
          )}

          <form onSubmit={handleSubmit} className="space-y-6">
            <div>
              <label htmlFor="name" className="block text-sm font-medium text-gray-700 mb-2">
                Full Name <span className="text-red-600">*</span>
              </label>
              <input
                id="name"
                name="name"
                type="text"
                required
                value={formData.name}
                onChange={handleChange}
                className="w-full px-4 py-3 border border-gray-300 rounded-md text-base text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
                placeholder="John Doe"
              />
            </div>

            <div>
              <label htmlFor="email" className="block text-sm font-medium text-gray-700 mb-2">
                Work email address <span className="text-red-600">*</span>
                {decodedEmail && (
                  <span className="text-xs text-gray-400 ml-1">
                    (Pre-filled from invitation)
                  </span>
                )}
              </label>
              <input
                id="email"
                name="email"
                type="email"
                autoComplete="email"
                required
                value={formData.email}
                onChange={handleChange}
                disabled={!isEmailEditable}
                className={`w-full px-4 py-3 border border-gray-300 rounded-md text-base text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors ${
                  !isEmailEditable ? 'opacity-50 cursor-not-allowed' : ''
                }`}
                placeholder="user@company.com"
              />
            </div>

            <div>
              <label htmlFor="joinCode" className="block text-sm font-medium text-gray-700 mb-2">
                Join Code <span className="text-red-600">*</span>
                {referralJoinCode && (
                  <span className="text-xs text-gray-400 ml-1">
                    (Pre-filled from invitation)
                  </span>
                )}
              </label>
              <input
                id="joinCode"
                name="join_code"
                type="text"
                required
                value={formData.join_code}
                onChange={handleChange}
                disabled={!isJoinCodeEditable}
                className={`w-full px-4 py-3 border border-gray-300 rounded-md text-base text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors ${
                  !isJoinCodeEditable ? 'opacity-50 cursor-not-allowed' : ''
                }`}
                placeholder="Enter your join code"
              />
              <p className="mt-1 text-xs text-gray-500">
                Ask your administrator for the join code to access your organization
              </p>
            </div>

            <div>
              <label htmlFor="password" className="block text-sm font-medium text-gray-700 mb-2">
                Password <span className="text-red-600">*</span>
              </label>
              <input
                id="password"
                name="password"
                type="password"
                autoComplete="new-password"
                required
                value={formData.password}
                onChange={handleChange}
                className="w-full px-4 py-3 border border-gray-300 rounded-md text-base text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
                placeholder="Minimum 6 characters"
              />
            </div>

            <div>
              <label htmlFor="confirmPassword" className="block text-sm font-medium text-gray-700 mb-2">
                Confirm Password <span className="text-red-600">*</span>
              </label>
              <input
                id="confirmPassword"
                name="confirmPassword"
                type="password"
                autoComplete="new-password"
                required
                value={formData.confirmPassword}
                onChange={handleChange}
                className="w-full px-4 py-3 border border-gray-300 rounded-md text-base text-gray-900 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
                placeholder="Confirm your password"
              />
            </div>

            <button
              type="submit"
              disabled={loading}
              className={`w-full py-3 px-4 rounded-md text-base font-medium transition-all ${
                loading
                  ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
                  : 'bg-blue-600 text-white hover:bg-blue-700 active:bg-blue-800'
              }`}
            >
              {loading 
                ? (isReferralLink ? 'Joining Organization...' : 'Creating Account...') 
                : (isReferralLink ? 'Join Organization' : 'Create Account')
              }
            </button>
          </form>

          {!isReferralLink && (
            <>
              <div className="relative my-6">
                <div className="absolute inset-0 flex items-center">
                  <div className="w-full border-t border-gray-300"></div>
                </div>
                <div className="relative flex justify-center text-sm">
                  <span className="px-2 bg-white text-gray-500">
                    Already have an account?
                  </span>
                </div>
              </div>

              <Link
                href="/login"
                className="block w-full text-center px-4 py-2 text-blue-600 font-medium hover:text-blue-700 transition-colors"
              >
                Sign in
              </Link>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default function UserRegisterPage() {
  return (
    <Suspense fallback={
      <div className="min-h-screen flex items-center justify-center bg-gray-50 px-6 py-12">
        <div className="text-center">
          <div className="w-12 h-12 border-4 border-gray-200 border-t-blue-600 rounded-full animate-spin mx-auto mb-4"></div>
          <p className="text-sm text-gray-600">Loading...</p>
        </div>
      </div>
    }>
      <UserRegisterPageContent />
    </Suspense>
  );
}
