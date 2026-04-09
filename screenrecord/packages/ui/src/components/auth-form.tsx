"use client";
import React, { useState } from "react";
import { ArrowRight, Eye, EyeOff } from "lucide-react";
import { cn } from "../utils/cn";
// --- HELPER COMPONENTS (ICONS) ---

const GoogleIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    className="h-5 w-5"
    viewBox="0 0 48 48"
  >
    <path
      fill="#FFC107"
      d="M43.611 20.083H42V20H24v8h11.303c-1.649 4.657-6.08 8-11.303 8-6.627 0-12-5.373-12-12s12-5.373 12-12c3.059 0 5.842 1.154 7.961 3.039l5.657-5.657C34.046 6.053 29.268 4 24 4 12.955 4 4 12.955 4 24s8.955 20 20 20 20-8.955 20-20c0-2.641-.21-5.236-.611-7.743z"
    />
    <path
      fill="#FF3D00"
      d="M6.306 14.691l6.571 4.819C14.655 15.108 18.961 12 24 12c3.059 0 5.842 1.154 7.961 3.039l5.657-5.657C34.046 6.053 29.268 4 24 4 16.318 4 9.656 8.337 6.306 14.691z"
    />
    <path
      fill="#4CAF50"
      d="M24 44c5.166 0 9.86-1.977 13.409-5.192l-6.19-5.238C29.211 35.091 26.715 36 24 36c-5.202 0-9.619-3.317-11.283-7.946l-6.522 5.025C9.505 39.556 16.227 44 24 44z"
    />
    <path
      fill="#1976D2"
      d="M43.611 20.083H42V20H24v8h11.303c-.792 2.237-2.231 4.166-4.087 5.571l6.19 5.238C42.022 35.026 44 30.038 44 24c0-2.641-.21-5.236-.611-7.743z"
    />
  </svg>
);

// --- SUB-COMPONENTS ---



interface GlassInputWrapperProps {
  children?: React.ReactNode;
  className?: string;
}
export const GlassInputWrapper = ({ children, className }: GlassInputWrapperProps) => (
  <div className={cn("rounded-2xl border border-border bg-foreground/5 backdrop-blur-sm transition-colors focus-within:border-primary/70 focus-within:bg-primary/10", className)}>
    {children}
  </div>
);

interface Testimonial {
  avatarSrc: string;
  name: string;
  handle: string;
  text: string;
}
interface TestimonialCardProps {
  testimonial: Testimonial;
  delay?: string;
}
const TestimonialCard = ({ testimonial, delay }: TestimonialCardProps) => (
  <div
    className={`animate-testimonial ${delay} flex items-start gap-3 rounded-3xl bg-card/90 backdrop-blur-xl border border-border p-5 w-64`}
  >
    <img
      src={testimonial.avatarSrc}
      className="h-10 w-10 object-cover rounded-2xl"
      alt="avatar"
    />
    <div className="text-sm leading-snug">
      <p className="flex items-center gap-1 font-medium">{testimonial.name}</p>
      <p className="text-muted-foreground">{testimonial.handle}</p>
      <p className="mt-1 text-foreground/80">{testimonial.text}</p>
    </div>
  </div>
);

// --- MAIN COMPONENT ---

interface AuthenticationFormProps {
  variant?: "signin" | "signup";
  title?: React.ReactNode;
  description?: React.ReactNode;
  heroImageSrc?: string;
  testimonials?: TestimonialCardProps[];
  onSignIn?: (event: React.FormEvent<HTMLFormElement>) => void;
  onSignUp?: (event: React.FormEvent<HTMLFormElement>) => void;
  onGoogleSignIn?: () => void;
  onResetPassword?: () => void;
  onCreateAccount?: () => void;
}
export const AuthenticationForm = ({
  variant = "signin",
  title,
  description,
  heroImageSrc,
  testimonials = [],
  onSignIn,
  onSignUp,
  onGoogleSignIn,
  onResetPassword,
  onCreateAccount,
}: AuthenticationFormProps) => {
  // TODO: Add early access drawer
  // const { openDrawer } = useEarlyAccess();
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirmPassword, setShowConfirmPassword] = useState(false);

  // Variant-based defaults
  const defaultTitle = variant === "signin" 
    ? <span className="font-light text-foreground tracking-tighter">Welcome</span>
    : <span className="font-light text-foreground tracking-tighter">Create Account</span>;

  const defaultDescription = true
    ? (
        <div className="flex items-center gap-2">
          <p>
            We are in early access. To get access to the platform{" "}
            <span 
            // TODO: Add early access drawer
            // onClick={() => openDrawer()} 
            onClick={() => console.log("Join the waitlist")} 
            
            className="text-primary cursor-pointer font-medium inline-flex items-center gap-1 hover:underline transition-colors">
              Join the waitlist <ArrowRight className="w-4 h-4" />
            </span>
          </p>
        </div>
      )
    : <p>Join thousands of users already using our platform</p>;

  const finalTitle = title || defaultTitle;
  const finalDescription = description || defaultDescription;

  // Form submission handler
  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (variant === "signin") {
      onSignIn?.(event);
    } else if (variant === "signup") {
      onSignUp?.(event);
    }
  };

  return (
    <div className="h-dvh flex flex-col md:flex-row font-geist w-dvw">
      {/* Left column: sign-in form */}
      <section className="flex-1 flex items-center justify-center p-8">
        <div className="w-full max-w-md">
          <div className="flex flex-col gap-6">
            <h1 className="animate-element animate-delay-100 text-4xl md:text-5xl font-semibold leading-tight">
              {finalTitle}
            </h1>
            <p className="animate-element animate-delay-200 text-muted-foreground">
              {finalDescription}
            </p>

            <form className="space-y-5" onSubmit={handleSubmit}>
              {variant === "signup" && (
                <div className="animate-element animate-delay-300">
                  <label className="text-sm font-medium text-muted-foreground">
                    Full Name
                  </label>
                  <GlassInputWrapper>
                    <input
                      name="name"
                      type="text"
                      placeholder="Enter your full name"
                      className="w-full bg-transparent text-sm p-4 rounded-2xl focus:outline-none"
                      required
                      minLength={2}
                    />
                  </GlassInputWrapper>
                </div>
              )}

              <div className={`animate-element ${variant === "signup" ? "animate-delay-400" : "animate-delay-300"}`}>
                <label className="text-sm font-medium text-muted-foreground">
                  Email Address
                </label>
                <GlassInputWrapper>
                  <input
                    name="email"
                    type="email"
                    placeholder="Enter your email address"
                    className="w-full bg-transparent text-sm p-4 rounded-2xl focus:outline-none"
                    required
                  />
                </GlassInputWrapper>
              </div>

              <div className={`animate-element ${variant === "signup" ? "animate-delay-500" : "animate-delay-400"}`}>
                <label className="text-sm font-medium text-muted-foreground">
                  Password
                </label>
                <GlassInputWrapper>
                  <div className="relative">
                    <input
                      name="password"
                      type={showPassword ? "text" : "password"}
                      placeholder="Enter your password"
                      className="w-full bg-transparent text-sm p-4 pr-12 rounded-2xl focus:outline-none"
                      required
                      minLength={8}
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute inset-y-0 right-3 flex items-center"
                    >
                      {showPassword ? (
                        <EyeOff className="w-5 h-5 text-muted-foreground hover:text-foreground transition-colors" />
                      ) : (
                        <Eye className="w-5 h-5 text-muted-foreground hover:text-foreground transition-colors" />
                      )}
                    </button>
                  </div>
                </GlassInputWrapper>
              </div>

              {variant === "signup" && (
                <div className="animate-element animate-delay-600">
                  <label className="text-sm font-medium text-muted-foreground">
                    Confirm Password
                  </label>
                  <GlassInputWrapper>
                    <div className="relative">
                      <input
                        name="confirmPassword"
                        type={showConfirmPassword ? "text" : "password"}
                        placeholder="Confirm your password"
                        className="w-full bg-transparent text-sm p-4 pr-12 rounded-2xl focus:outline-none"
                        required
                        minLength={8}
                      />
                      <button
                        type="button"
                        onClick={() => setShowConfirmPassword(!showConfirmPassword)}
                        className="absolute inset-y-0 right-3 flex items-center"
                      >
                        {showConfirmPassword ? (
                          <EyeOff className="w-5 h-5 text-muted-foreground hover:text-foreground transition-colors" />
                        ) : (
                          <Eye className="w-5 h-5 text-muted-foreground hover:text-foreground transition-colors" />
                        )}
                      </button>
                    </div>
                  </GlassInputWrapper>
                </div>
              )}

              {variant === "signin" && (
                <div className="animate-element animate-delay-500 flex items-center justify-between text-sm">
                <label className="flex items-center gap-3 cursor-pointer">
                  <input
                    type="checkbox"
                    name="rememberMe"
                    className="sr-only"
                    onChange={() => {
                      console.log("Remember me clicked");
                    }}
                  />
                  <span className="custom-checkbox"></span>
                  <span className="text-foreground/90">Keep me signed in</span>
                </label>
                <a
                  href="#"
                  onClick={(e) => {
                    e.preventDefault();
                    onResetPassword?.();
                  }}
                className="hover:underline text-primary transition-colors"
                >
                  Reset password
                </a>
                </div>
              )}

              {variant === "signup" && (
                <div className="animate-element animate-delay-700">
                  <label className="flex items-center gap-3 cursor-pointer">
                    <input
                      type="checkbox"
                      name="acceptTerms"
                      className="sr-only"
                      required
                      onChange={() => {
                        console.log("Terms accepted");
                      }}
                    />
                    <span className="custom-checkbox"></span>
                    <span className="text-foreground/90 text-sm">
                      I agree to the{" "}
                      <a
                        href="/terms"
                        className="text-primary hover:underline transition-colors"
                      >
                        Terms of Service
                      </a>{" "}
                      and{" "}
                      <a
                        href="/privacy"
                        className="text-primary hover:underline transition-colors"
                      >
                        Privacy Policy
                      </a>
                    </span>
                  </label>
                </div>
              )}

              <button
                type="submit"
                className={`animate-element ${variant === "signup" ? "animate-delay-800" : "animate-delay-600"} w-full rounded-2xl bg-primary py-4 font-medium text-primary-foreground hover:bg-primary/90 transition-colors cursor-pointer`}
              >
                {variant === "signin" ? "Sign In" : "Create Account"}
              </button>
            </form>

            <div className={`animate-element ${variant === "signup" ? "animate-delay-900" : "animate-delay-700"} relative flex items-center justify-center`}>
              <span className="w-full border-t border-border"></span>
              <span className="px-4 text-sm text-muted-foreground bg-background absolute">
                Or continue with
              </span>
            </div>

            <button
              onClick={onGoogleSignIn}
              className={`animate-element ${variant === "signup" ? "animate-delay-1000" : "animate-delay-800"} w-full flex items-center justify-center gap-3 border border-border rounded-2xl py-4 hover:bg-secondary transition-colors cursor-pointer`}
            >
              <GoogleIcon />
              Continue with Google
            </button>

            <p className={`animate-element ${variant === "signup" ? "animate-delay-1100" : "animate-delay-900"} text-center text-sm text-muted-foreground`}>
              {variant === "signin" ? (
                <>
                  New to our platform?{" "}
                  <a
                    href="/sign-up"
                    className="text-primary hover:underline transition-colors"
                  >
                    Create Account
                  </a>
                </>
              ) : (
                <>
                  Already have an account?{" "}
                  <a
                    href="/sign-in"
                    className="text-primary hover:underline transition-colors"
                  >
                    Sign In
                  </a>
                </>
              )}
            </p>
          </div>
        </div>
      </section>
      {/* Right column: hero image + testimonials */}
      {heroImageSrc && (
        <section className="hidden md:block flex-1 relative p-4">
          <div
            className="flex justify-center items-center bg-[radial-gradient(125%_125%_at_50%_101%,rgba(245,87,2,1)_10.5%,rgba(245,120,2,1)_16%,rgba(245,140,2,1)_17.5%,rgba(245,170,100,1)_25%,rgba(238,174,202,1)_40%,rgba(202,179,214,1)_65%,rgba(148,201,233,1)_100%)] absolute inset-4 rounded-3xl bg-cover bg-cente"
            // className="animate-slide-right animate-delay-300 absolute inset-4 rounded-3xl bg-cover bg-center"
            // style={{ backgroundImage: `url(${heroImageSrc})`, backdropFilter: "blur(100px)", backgroundBlendMode: "overlay" }}
          />
          {testimonials.length > 0 && (
            <div className="absolute bottom-8 left-1/2 -translate-x-1/2 flex gap-4 px-8 w-full justify-center">
              {/* <div className="p-4 w-[500px]">
                <PromptInputBox
                  onSend={(message, files) => console.log(message, files)}
                />
              </div> */}
              <TestimonialCard testimonial={testimonials[0] as unknown as Testimonial} delay="animate-delay-1000" />
              {testimonials[1] && <div className="hidden xl:flex"><TestimonialCard testimonial={testimonials[1] as unknown as Testimonial} delay="animate-delay-1200" /></div>}
              {testimonials[2] && <div className="hidden 2xl:flex"><TestimonialCard testimonial={testimonials[2] as unknown as Testimonial} delay="animate-delay-1400" /></div>}
            </div>
          )}
        </section>
      )}
    </div>
  );
};

// Backward compatibility export
export const SignInPage = AuthenticationForm;
